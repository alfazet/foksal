# Foksal

A remote music player. Perfect for those who enjoy owning their music.

## Installation
Install from cargo with `cargo install foksal-{TYPE}` (where `TYPE` is `local`, `remote` or `proxy`) or download the source code and build foksal locally.
Minimum supported Rust version (MSRV): 1.90.

## Usage
Quite simple, although it depends on your setup.
Below, `music_root` denotes the path to the root directory of your music collection.

### Run locally
If your music collection is stored locally, run `foksal-local -m=<music_root>`.

### Run remotely
Set up a remote foksal instance by running `foksal-remote -m=<music_root>` on the server where your music is stored.
Then, on your local machine connect to the remote instance with `foksal-proxy --remote-addr=<ip_address>`.

To see all available options for a given binary, run it with the `--help` flag.

## Configuration
Foksal (in all three versions) reads its configuration from a TOML config file, by default `~/.config/foksal/foksal.toml` (change the path with the `-c` flag if needed).
If the config file isn't there, it will be generated based on the default config and passed CLI arguments. All available options are listed below:

### Local options
- `port` - The port for clients to connect to.
- `music_root` - The root directory of your music collection.
- `audio_backend` - The audio backend (alsa, pulse, pipewire, etc.), it's very likely you should keep it at `default`.
- `log_file` - The location of foksal's log file.

### Remote options
- `port`, `music_root` and `log_file` - Same meaning as in local config (`port` is the port that proxy instances should connect to).

### Proxy options
- `remote_addr` - The IP address of the remote foksal instance.
- `remote_port` - The port the remote instance is listening on.
- `local_port` - The port for clients to connect to.
- `audio_backend` and `log_file` - Same meaning as in local config.
