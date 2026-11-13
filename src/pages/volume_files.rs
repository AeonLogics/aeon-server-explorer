use leptos::prelude::*;
use shared::volumes::{join_path, valid_file_name};

use crate::api::volumes::{list_files, other_volumes, CopyInto, DeletePath, MakeFolder};
use crate::components::icon::Ico;
use crate::components::stats::bytes;
use crate::components::upload::{picked_files, send_file};

const FILE_TYPE: &str = "application/x-explorer-file";

// Percent-encoding for query strings: everything but unreserved characters.
fn enc(value: &str) -> String {
    value
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

// Browse a storage's files, upload into it, download, make folders, delete, and copy in
// from another volume. Every step runs in a throwaway helper container on the server.
#[component]
pub fn VolumeFiles(stack: Memo<String>, storage: String) -> impl IntoView {
    let storage = StoredValue::new(storage);
    let path = RwSignal::new(String::new());
    let refresh = RwSignal::new(0u32);
    let listing = Resource::new(
        move || (stack.get(), storage.get_value(), path.get(), refresh.get()),
        |(stack, storage, path, _)| list_files(stack, storage, path),
    );
    let others = Resource::new(
        move || (stack.get(), storage.get_value()),
        |(stack, storage)| other_volumes(stack, storage),
    );

    let note = RwSignal::new(None::<(bool, String)>);
    let uploading = RwSignal::new(None::<String>);
    let folder_name = RwSignal::new(None::<String>);
    let asking_delete = RwSignal::new(None::<String>);
    let copy_from = RwSignal::new(String::new());
    let asking_copy = RwSignal::new(false);

    let mkdir = ServerAction::<MakeFolder>::new();
    let remove = ServerAction::<DeletePath>::new();
    let copy = ServerAction::<CopyInto>::new();

    Effect::new(move |_| match mkdir.value().get() {
        Some(Ok(())) => {
            folder_name.set(None);
            refresh.update(|r| *r += 1);
        }
        Some(Err(e)) => note.set(Some((false, e.to_string()))),
        None => {}
    });
    Effect::new(move |_| match remove.value().get() {
        Some(Ok(())) => refresh.update(|r| *r += 1),
        Some(Err(e)) => note.set(Some((false, e.to_string()))),
        None => {}
    });
    Effect::new(move |_| match copy.value().get() {
        Some(Ok(n)) => {
            note.set(Some((true, format!("Copied {n} files in."))));
            refresh.update(|r| *r += 1);
        }
        Some(Err(e)) => note.set(Some((false, e.to_string()))),
        None => {}
    });

    let file_url = move |name: &str, inline: bool| {
        format!(
            "/api/files/download?stack={}&storage={}&path={}{}",
            enc(&stack.get_untracked()),
            enc(&storage.get_value()),
            enc(&join_path(&path.get_untracked(), name)),
            if inline { "&inline=true" } else { "" }
        )
    };
    let all_url = move || {
        format!(
            "/api/files/download?kind=all&stack={}&storage={}&path={}",
            enc(&stack.get()),
            enc(&storage.get_value()),
            enc(&path.get())
        )
    };

    let busy = move || uploading.with(Option::is_some) || copy.pending().get();

    let on_pick = move |ev: leptos::ev::Event| {
        let files = picked_files(&ev);
        if files.is_empty() {
            return;
        }
        note.set(None);
        let (stack_name, storage_name, dir) = (stack.get_untracked(), storage.get_value(), path.get_untracked());
        leptos::task::spawn_local(async move {
            let total = files.len();
            let mut done = 0;
            let mut problems = Vec::new();
            for (i, file) in files.iter().enumerate() {
                let name = file.name();
                if !valid_file_name(&name) {
                    problems.push(format!("{name}: the name is too long or has a slash"));
                    continue;
                }
                let label = format!("Uploading {} of {total}: {name}", i + 1);
                uploading.try_set(Some(label.clone()));
                let url = format!(
                    "/api/files/upload?stack={}&storage={}&dir={}&name={}",
                    enc(&stack_name),
                    enc(&storage_name),
                    enc(&dir),
                    enc(&name)
                );
                let progress_label = label.clone();
                let result = send_file(file, &url, FILE_TYPE, move |sent, size| {
                    if size > 0.0 {
                        uploading.try_set(Some(format!("{progress_label} · {:.0}%", sent / size * 100.0)));
                    }
                })
                .await;
                match result {
                    Ok(_) => done += 1,
                    Err(e) => problems.push(format!("{name}: {e}")),
                }
            }
            uploading.try_set(None);
            refresh.try_update(|r| *r += 1);
            let message = if problems.is_empty() {
                (true, if done == 1 { "Uploaded 1 file.".to_string() } else { format!("Uploaded {done} files.") })
            } else {
                (false, format!("Uploaded {done} of {total}. {}", problems.join(" · ")))
            };
            note.try_set(Some(message));
        });
    };

    let crumbs = move || {
        let current = path.get();
        let mut out = vec![(storage.get_value(), String::new())];
        let mut so_far = String::new();
        for part in current.split('/').filter(|p| !p.is_empty()) {
            so_far = join_path(&so_far, part);
            out.push((part.to_string(), so_far.clone()));
        }
        out
    };

    view! {
        <section class="files" aria-label="Files">
            <div class="files__bar">
                <nav class="files__crumbs" aria-label="Folder">
                    {move || {
                        crumbs()
                            .into_iter()
                            .enumerate()
                            .map(|(i, (label, target))| {
                                view! {
                                    {(i > 0).then(|| view! { <span class="files__sep" aria-hidden="true">"/"</span> })}
                                    <button type="button" class="files__crumb" on:click=move |_| {
                                        asking_delete.set(None);
                                        path.set(target.clone());
                                    }>
                                        {label}
                                    </button>
                                }
                            })
                            .collect_view()
                    }}
                </nav>
                <div class="files__actions">
                    <button type="button" class="tool-btn" on:click=move |_| folder_name.set(Some(String::new()))>
                        <Ico icon=icondata::LuFolderPlus />
                        "New folder"
                    </button>
                    <label class="tool-btn files__upload" class:is-disabled=busy>
                        <Ico icon=icondata::LuUpload />
                        "Upload files"
                        <input type="file" multiple=true class="sr-only" disabled=busy on:change=on_pick />
                    </label>
                    <a class="tool-btn" href=all_url download="">
                        <Ico icon=icondata::LuDownload />
                        "Download all"
                    </a>
                </div>
            </div>

            {move || {
                folder_name.get().map(|typed| {
                    view! {
                        <form
                            class="files__new"
                            on:submit=move |ev: leptos::ev::SubmitEvent| {
                                ev.prevent_default();
                                let name = folder_name.get_untracked().unwrap_or_default().trim().to_string();
                                if name.is_empty() || name.contains('/') {
                                    return;
                                }
                                mkdir.dispatch(MakeFolder {
                                    stack: stack.get_untracked(),
                                    storage: storage.get_value(),
                                    path: join_path(&path.get_untracked(), &name),
                                });
                            }
                        >
                            <input
                                class="field__input field__input--plain"
                                type="text"
                                placeholder="folder name"
                                autocomplete="off"
                                spellcheck="false"
                                prop:value=typed
                                on:input=move |ev| folder_name.set(Some(event_target_value(&ev)))
                            />
                            <button type="submit" class="tool-btn" disabled=move || mkdir.pending().get()>"Create"</button>
                            <button type="button" class="tool-btn" on:click=move |_| folder_name.set(None)>"Cancel"</button>
                        </form>
                    }
                })
            }}

            {move || uploading.get().map(|text| view! { <p class="files__status">{text}</p> })}
            {move || {
                note.get().map(|(ok, text)| {
                    view! { <p class="flash" class:is-ok=ok class:is-err=!ok>{text}</p> }
                })
            }}

            <Transition fallback=|| view! { <div class="skeleton skeleton--block"></div> }>
                {move || {
                    listing.get().map(|result| match result {
                        Err(e) => view! { <p class="inspector__hint">{e.to_string()}</p> }.into_any(),
                        Ok(list) if list.entries.is_empty() => view! {
                            <p class="inspector__hint">"This folder is empty. Upload files, or copy them in from another volume below."</p>
                        }
                        .into_any(),
                        Ok(list) => view! {
                            <ul class="files__list">
                                {list
                                    .entries
                                    .into_iter()
                                    .map(|entry| {
                                        let name = entry.name.clone();
                                        let for_ask = name.clone();
                                        let for_is_asking = name.clone();
                                        let for_delete = name.clone();
                                        let is_asking = move || asking_delete.with(|a| a.as_deref() == Some(for_is_asking.as_str()));
                                        let open = if entry.is_dir {
                                            let into = name.clone();
                                            view! {
                                                <button type="button" class="files__name" on:click=move |_| {
                                                    asking_delete.set(None);
                                                    path.update(|p| *p = join_path(p, &into));
                                                }>
                                                    <Ico icon=icondata::LuFolder />
                                                    {name.clone()}
                                                </button>
                                            }
                                            .into_any()
                                        } else {
                                            view! {
                                                <a class="files__name" href=file_url(&name, true) target="_blank" rel="noopener">
                                                    <Ico icon=icondata::LuFile />
                                                    {name.clone()}
                                                </a>
                                            }
                                            .into_any()
                                        };
                                        let download = (!entry.is_dir).then(|| {
                                            view! {
                                                <a class="icon-btn" href=file_url(&name, false) download="" aria-label="Download" title="Download">
                                                    <Ico icon=icondata::LuDownload />
                                                </a>
                                            }
                                        });
                                        let is_dir = entry.is_dir;
                                        view! {
                                            <li class="files__row">
                                                {open}
                                                <span class="files__size">{if entry.is_dir { String::new() } else { bytes(entry.size as f64) }}</span>
                                                <span class="files__date">{entry.modified.clone()}</span>
                                                <span class="files__row-actions">
                                                    <Show
                                                        when=is_asking.clone()
                                                        fallback=move || {
                                                            let ask = for_ask.clone();
                                                            view! {
                                                                <button type="button" class="icon-btn" aria-label="Delete" title="Delete" on:click=move |_| asking_delete.set(Some(ask.clone()))>
                                                                    <Ico icon=icondata::LuTrash2 />
                                                                </button>
                                                            }
                                                        }
                                                    >
                                                        <span class="images-table__confirm">
                                                            {if is_dir { "Delete folder and all inside? " } else { "Delete? " }}
                                                            <button
                                                                type="button"
                                                                class="tool-btn is-danger"
                                                                on:click={
                                                                    let target = for_delete.clone();
                                                                    move |_| {
                                                                        asking_delete.set(None);
                                                                        remove.dispatch(DeletePath {
                                                                            stack: stack.get_untracked(),
                                                                            storage: storage.get_value(),
                                                                            path: join_path(&path.get_untracked(), &target),
                                                                        });
                                                                    }
                                                                }
                                                            >
                                                                "Delete"
                                                            </button>
                                                            <button type="button" class="tool-btn" on:click=move |_| asking_delete.set(None)>"Cancel"</button>
                                                        </span>
                                                    </Show>
                                                    {download}
                                                </span>
                                            </li>
                                        }
                                    })
                                    .collect_view()}
                            </ul>
                        }
                        .into_any(),
                    })
                }}
            </Transition>

            <div class="files__copy">
                <span class="field__label">"COPY IN FROM ANOTHER VOLUME"</span>
                <div class="files__copy-row">
                    <Transition fallback=|| view! { <select class="field__input field__input--plain" disabled=true></select> }>
                        {move || {
                            let list = others.get().and_then(Result::ok).unwrap_or_default();
                            view! {
                                <select
                                    class="field__input field__input--plain"
                                    prop:value=move || copy_from.get()
                                    on:change=move |ev| {
                                        asking_copy.set(false);
                                        copy_from.set(event_target_value(&ev));
                                    }
                                >
                                    <option value="">"Pick a volume on this server"</option>
                                    {list.into_iter().map(|v| view! { <option value=v.clone()>{v.clone()}</option> }).collect_view()}
                                </select>
                            }
                        }}
                    </Transition>
                    <button
                        type="button"
                        class="tool-btn"
                        disabled={move || copy_from.with(String::is_empty) || busy()}
                        on:click=move |_| asking_copy.set(true)
                    >
                        <Ico icon=icondata::LuCopy />
                        {move || if copy.pending().get() { "Copying…" } else { "Copy in" }}
                    </button>
                </div>
                <Show when=move || asking_copy.get()>
                    <p class="files__confirm">
                        {move || format!(
                            "Copy everything from {} into {}? Files with the same name are replaced, and {} itself isn't changed.",
                            copy_from.get(),
                            storage.get_value(),
                            copy_from.get()
                        )}
                        <button
                            type="button"
                            class="tool-btn"
                            on:click=move |_| {
                                asking_copy.set(false);
                                note.set(None);
                                copy.dispatch(CopyInto {
                                    stack: stack.get_untracked(),
                                    storage: storage.get_value(),
                                    from: copy_from.get_untracked(),
                                });
                            }
                        >
                            "Copy"
                        </button>
                        <button type="button" class="tool-btn" on:click=move |_| asking_copy.set(false)>"Cancel"</button>
                    </p>
                </Show>
            </div>
        </section>
    }
}
