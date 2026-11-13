use shared::stack::{DEFAULT_NETWORK, ServiceSpec, StackSpec, own_volume_key};

pub const HEADER: &str = "# Managed by Aeon Server Explorer. Edit this stack in the app: changes made here are overwritten.\n";
pub const LABEL_MANAGED: &str = "dev.aeonlogics.explorer.managed";
pub use shared::docker::LABEL_RUN_ONCE;

pub fn env_file(service: &str) -> String {
    format!("env/{service}.env")
}

// Every value written here was validated against a strict character set (shared::stack),
// so plain quoting is enough and nothing can break out of its YAML scalar.
pub fn render(spec: &StackSpec) -> String {
    let mut out = String::from(HEADER);
    out.push_str(&format!("name: {}\n\n", spec.name));

    if spec.services.is_empty() {
        out.push_str("services: {}\n");
        return out;
    }

    out.push_str("services:\n");
    for service in &spec.services {
        render_service(&mut out, service);
    }

    let volumes: Vec<String> = spec
        .services
        .iter()
        .flat_map(|s| s.volumes.iter().map(move |v| own_volume_key(&s.name, &v.name)))
        .chain(spec.storages.iter().map(|s| s.name.clone()))
        .collect();
    if !volumes.is_empty() {
        out.push_str("\nvolumes:\n");
        for key in volumes {
            out.push_str(&format!("  {key}: {{}}\n"));
        }
    }

    if !spec.networks.is_empty() {
        out.push_str("\nnetworks:\n");
        for n in &spec.networks {
            out.push_str(&format!("  {}: {{}}\n", n.name));
        }
    }
    out
}

// Command parts are free text, so each becomes a fully escaped double-quoted YAML string.
// `$` is doubled because compose would otherwise treat it as a variable.
fn yaml_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '$' => out.push_str("$$"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn render_service(out: &mut String, s: &ServiceSpec) {
    out.push_str(&format!("  {}:\n", s.name));
    out.push_str(&format!("    image: \"{}\"\n", s.image));
    let restart = if s.run_once { "\"no\"" } else { s.restart.compose() };
    out.push_str(&format!("    restart: {restart}\n"));
    if let Some(user) = &s.user {
        out.push_str(&format!("    user: \"{user}\"\n"));
    }
    if !s.command.is_empty() {
        let args: Vec<String> = s.command.iter().map(|a| yaml_string(a)).collect();
        out.push_str(&format!("    command: [{}]\n", args.join(", ")));
    }
    out.push_str("    labels:\n");
    out.push_str(&format!("      {LABEL_MANAGED}: \"true\"\n"));
    if s.run_once {
        out.push_str(&format!("      {LABEL_RUN_ONCE}: \"true\"\n"));
    }

    if !s.depends_on.is_empty() {
        out.push_str("    depends_on:\n");
        for d in &s.depends_on {
            out.push_str(&format!("      {}:\n", d.service));
            out.push_str(&format!("        condition: {}\n", d.condition.compose()));
        }
    }

    if !s.env.is_empty() {
        out.push_str("    env_file:\n");
        out.push_str(&format!("      - path: ./{}\n", env_file(&s.name)));
        out.push_str("        format: raw\n");
    }

    let published: Vec<_> = s
        .ports
        .iter()
        .filter_map(|p| p.host.map(|h| (if p.public { "0.0.0.0" } else { "127.0.0.1" }, h, p.container)))
        .collect();
    if !published.is_empty() {
        out.push_str("    ports:\n");
        for (ip, host, container) in published {
            out.push_str(&format!("      - \"{ip}:{host}:{container}\"\n"));
        }
    }

    let internal: Vec<_> = s.ports.iter().filter(|p| p.host.is_none()).map(|p| p.container).collect();
    if !internal.is_empty() {
        out.push_str("    expose:\n");
        for port in internal {
            out.push_str(&format!("      - \"{port}\"\n"));
        }
    }

    if !s.volumes.is_empty() || !s.mounts.is_empty() {
        out.push_str("    volumes:\n");
        for v in &s.volumes {
            out.push_str(&format!("      - \"{}:{}\"\n", own_volume_key(&s.name, &v.name), v.path));
        }
        for m in &s.mounts {
            let mode = if m.read_only { ":ro" } else { "" };
            out.push_str(&format!("      - \"{}:{}{mode}\"\n", m.storage, m.path));
        }
    }

    // Leaving it out keeps compose's default network, so stacks without extra networks
    // render exactly as before.
    if !s.networks.is_empty() && s.networks != [DEFAULT_NETWORK] {
        out.push_str("    networks:\n");
        for n in &s.networks {
            out.push_str(&format!("      - {n}\n"));
        }
    }
}
