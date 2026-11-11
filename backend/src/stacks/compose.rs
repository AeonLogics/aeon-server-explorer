use shared::stack::{ServiceSpec, StackSpec};

pub const HEADER: &str = "# Managed by Aeon Server Explorer. Edit this stack in the app: changes made here are overwritten.\n";
pub const LABEL_MANAGED: &str = "dev.aeonlogics.explorer.managed";

pub fn volume_key(service: &str, volume: &str) -> String {
    format!("{service}-{volume}")
}

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
        .flat_map(|s| s.volumes.iter().map(move |v| volume_key(&s.name, &v.name)))
        .collect();
    if !volumes.is_empty() {
        out.push_str("\nvolumes:\n");
        for key in volumes {
            out.push_str(&format!("  {key}: {{}}\n"));
        }
    }
    out
}

fn render_service(out: &mut String, s: &ServiceSpec) {
    out.push_str(&format!("  {}:\n", s.name));
    out.push_str(&format!("    image: \"{}\"\n", s.image));
    out.push_str(&format!("    restart: {}\n", s.restart.compose()));
    out.push_str("    labels:\n");
    out.push_str(&format!("      {LABEL_MANAGED}: \"true\"\n"));

    if !s.env.is_empty() {
        out.push_str("    env_file:\n");
        out.push_str(&format!("      - path: ./{}\n", env_file(&s.name)));
        out.push_str("        format: raw\n");
    }

    let published: Vec<_> = s.ports.iter().filter_map(|p| p.host.map(|h| (h, p.container))).collect();
    if !published.is_empty() {
        out.push_str("    ports:\n");
        for (host, container) in published {
            out.push_str(&format!("      - \"127.0.0.1:{host}:{container}\"\n"));
        }
    }

    let internal: Vec<_> = s.ports.iter().filter(|p| p.host.is_none()).map(|p| p.container).collect();
    if !internal.is_empty() {
        out.push_str("    expose:\n");
        for port in internal {
            out.push_str(&format!("      - \"{port}\"\n"));
        }
    }

    if !s.volumes.is_empty() {
        out.push_str("    volumes:\n");
        for v in &s.volumes {
            out.push_str(&format!("      - \"{}:{}\"\n", volume_key(&s.name, &v.name), v.path));
        }
    }
}
