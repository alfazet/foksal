# Foksal

A remote music player with MPRIS support. Perfect for those who enjoy owning their music.

## Installation
Install from cargo with `cargo install foksal-{TYPE}` (where `TYPE` is one of `local`, `remote` or `proxy`, see below) or download the source code and build foksal locally.
Minimum supported Rust version (MSRV): 1.90.

## Usage
In the descriptions below, `music_root` denotes the path (relative or absolute) to the root directory of your music collection.

### Run locally
If your music collection is stored locally, run `foksal-local -m=<music_root>`.

### Run remotely
Set up a remote foksal instance by running `foksal-remote -m=<music_root>` on the server where your music is stored.
Then, on your local machine connect to the remote instance with `foksal-proxy --remote-addr=<ip_address>`.

To see all available options for a given binary, run it with the `--help` flag.

> You will most likely want foksal to start at boot and run as a background process (as a systemd unit, for example).

## Configuration
Foksal reads its configuration from a TOML config file, by default `~/.config/foksal/foksal.toml` (change the path with the `-c` flag if needed).
If the config file isn't there, it will be generated based on the default config and passed CLI arguments. All available options are listed below:

### Local options
- `port` - The port for clients to connect to (default: `2137`).
- `music_root` - The root directory of your music collection (default: foksal's working dir).
- `audio_backend` - The audio backend (alsa, pulse, pipewire, etc.) (default: well, 1default` - it should work just fine).
- `allowed_exts` - A list of extensions that foksal will treat as music files (default: `mp3`, `m4a` and `flac`).
- `n_jobs` - How many songs can be decoded in parallel (default: the number of available CPU cores). Careful: each decoded song requires a chunk of RAM.
- `ignore_globset` - A list of Unix glob patterns that foksal will ignore when searching for music (default: empty).

### Remote options
- `interface` - The interface the remote instance will listen on (default: `0.0.0.0` (all interfaces)).
- `port`, `music_root`, `allowed_exts`, `n_jobs` and `ignore_globset` - Same meaning as in local config (`port` is the port that proxy instances should connect to, make sure it's accessible).

### Proxy options
- `remote_addr` - The IP address of the remote foksal instance.
- `remote_port` - The port the remote instance is listening on.
- `local_port` - The port for clients to connect to.
- `audio_backend` - Same meaning as in local config.

Keep in mind that all foksal binaries read the same config file by default (meaning that, as an example, you don't need two almost-duplicated config file for `local` and `proxy` - 
if `proxy` reads a config key it doesn't need, it will just silently ignore it).

## Clients
Foksal isn't very useful all by itself, you will need an external client to control it.

For now, only a basic CLI-based client is available, install it with `cargo install foksal-ctl`. It's not very human-friendly, but can work well
as part of a scripting pipeline (for instance, to display the current song on your statusbar).

Another option is to control foksal with [playerctl](https://github.com/altdesktop/playerctl), just like any other MPRIS-compatible player (for now only `foksal-local`, `proxy` support coming soon).

A [GUI client](https://codeberg.org/alfazet/feux) will be released soon.

## Roadmap
Currently planned features:
- Data persistence between restarts.
- Support for user-specific data (ratings, comments, ...).
- Support for song lyrics.
- Support for frequency spectrum visualization.

## Contributing
Want to contribute? Create an [issue](https://codeberg.org/alfazet/foksal/issues) on Codeberg (that's where foksal's development happens, the parallel GitHub repo is just a mirror).

## Trivia
Foksal (/ˈfɔk.sal/) is a [street](https://en.wikipedia.org/wiki/Foksal_Street) in my hometown. Also, it's no secret that software with foxes is the best.
