//! Independent desktop audio service; never starts the legacy GG audio loop.
#[cfg(target_os = "linux")]
fn main() -> anyhow::Result<()> {
    use anyhow::{Context, anyhow};
    use clap::{Parser, ValueEnum};
    use std::os::unix::fs::FileTypeExt;
    use std::path::PathBuf;
    use steelseries_gg::desktop::{Backend, Service, Snapshot, pipewire::PipeWireBackend, pulse::PulseBackend, rpc};
    #[derive(Clone, Copy, ValueEnum)]
    enum AudioBackendChoice {
        Pulse,
        Pipewire,
    }
    enum SelectedBackend {
        Pulse(PulseBackend),
        Pipewire(PipeWireBackend),
    }
    impl Backend for SelectedBackend {
        fn backend_name(&self) -> &'static str {
            match self {
                Self::Pulse(b) => b.backend_name(),
                Self::Pipewire(b) => b.backend_name(),
            }
        }
        fn begin_cycle(&mut self) {
            match self {
                Self::Pulse(b) => b.begin_cycle(),
                Self::Pipewire(b) => b.begin_cycle(),
            }
        }
        fn snapshot(&mut self) -> Result<Snapshot, String> {
            match self {
                Self::Pulse(b) => b.snapshot(),
                Self::Pipewire(b) => b.snapshot(),
            }
        }
        fn set_stream(
            &mut self,
            id: u32,
            volume: Option<f64>,
            muted: Option<bool>,
            sink: Option<u32>,
        ) -> Result<(), String> {
            match self {
                Self::Pulse(b) => b.set_stream(id, volume, muted, sink),
                Self::Pipewire(b) => b.set_stream(id, volume, muted, sink),
            }
        }
        fn set_stream_guarded(
            &mut self,
            id: u32,
            volume: Option<f64>,
            muted: Option<bool>,
            sink: Option<u32>,
            allowed: &dyn Fn() -> bool,
        ) -> Result<(), String> {
            match self {
                Self::Pulse(b) => b.set_stream_guarded(id, volume, muted, sink, allowed),
                Self::Pipewire(b) => b.set_stream_guarded(id, volume, muted, sink, allowed),
            }
        }
        fn set_stream_guarded_owned(
            &mut self,
            id: u32,
            volume: Option<f64>,
            muted: Option<bool>,
            sink: Option<u32>,
            allowed: std::sync::Arc<dyn Fn() -> bool + Send + Sync>,
        ) -> Result<(), String> {
            match self {
                Self::Pulse(b) => b.set_stream_guarded(id, volume, muted, sink, &*allowed),
                Self::Pipewire(b) => b.set_stream_guarded_owned(id, volume, muted, sink, allowed),
            }
        }
    }
    #[derive(Parser)]
    #[command(
        name = "ssgg-desktop",
        about = "Private desktop audio RPC service (native PipeWire when its socket exists; otherwise Pulse)"
    )]
    struct Args {
        #[arg(long, value_enum)]
        audio_backend: Option<AudioBackendChoice>,
        #[arg(long, requires = "audio_backend")]
        pipewire_runtime: Option<PathBuf>,
        #[arg(long)]
        stdio: bool,
        #[arg(long)]
        safe_mode: bool,
        #[arg(long)]
        socket: Option<PathBuf>,
        #[arg(long)]
        config: Option<PathBuf>,
    }
    let args = Args::parse();
    let config = match args.config {
        Some(path) => path,
        None => std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .ok_or_else(|| anyhow!("HOME or XDG_CONFIG_HOME required"))?
            .join("ssgg-desktop/state.json"),
    };
    let native_socket = std::env::var_os("PIPEWIRE_RUNTIME_DIR")
        .or_else(|| std::env::var_os("XDG_RUNTIME_DIR"))
        .map(|runtime| {
            PathBuf::from(runtime).join(std::env::var_os("PIPEWIRE_REMOTE").unwrap_or_else(|| "pipewire-0".into()))
        });
    let choice = args.audio_backend.unwrap_or_else(|| {
        if native_socket
            .as_ref()
            .is_some_and(|socket| std::fs::metadata(socket).is_ok_and(|metadata| metadata.file_type().is_socket()))
        {
            AudioBackendChoice::Pipewire
        } else {
            AudioBackendChoice::Pulse
        }
    });
    let backend = match choice {
        AudioBackendChoice::Pulse => {
            if args.pipewire_runtime.is_some() {
                return Err(anyhow!("--pipewire-runtime requires --audio-backend pipewire"));
            }
            SelectedBackend::Pulse(PulseBackend::default())
        }
        AudioBackendChoice::Pipewire => {
            let mut backend = if let Some(runtime) = args.pipewire_runtime.as_ref() {
                PipeWireBackend::connect_at(runtime, "pipewire-0", std::time::Duration::from_secs(3))
            } else {
                PipeWireBackend::from_environment()
            }
            .map_err(|e| anyhow!(e))?;
            // Fail before creating a service if the selected private socket is unavailable.
            backend
                .snapshot()
                .map_err(|e| anyhow!("native PipeWire unavailable: {e}"))?;
            SelectedBackend::Pipewire(backend)
        }
    };
    let mut service = Service::new(backend, config).map_err(|e| anyhow!(e))?;
    service.set_read_only(args.safe_mode);
    let (client, join) = rpc::worker(service);
    if args.stdio {
        let result = rpc::serve_lines(&mut std::io::stdin().lock(), &mut std::io::stdout().lock(), |value| {
            client.request(value)
        });
        drop(client);
        join.join().map_err(|_| anyhow!("desktop worker failed"))?;
        result.context("stdio RPC")?;
    } else {
        let socket = match args.socket {
            Some(path) => path,
            None => PathBuf::from(
                std::env::var_os("XDG_RUNTIME_DIR")
                    .ok_or_else(|| anyhow!("XDG_RUNTIME_DIR required; use --stdio or --socket"))?,
            )
            .join("ssgg-desktop/service.sock"),
        };
        rpc::serve_socket(&socket, client).context("private desktop socket")?;
    }
    Ok(())
}
#[cfg(not(target_os = "linux"))]
fn main() -> anyhow::Result<()> {
    anyhow::bail!("ssgg-desktop audio service currently requires Linux/Unix")
}
