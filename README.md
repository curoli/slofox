# slofox

A Rust/Bevy app that renders two cartoon talk show hosts in a 3D studio.
Host 1 reacts to ChatGPT's browser audio; host 2 reacts to your microphone.
OBS records the app window and the two audio channels.

![Prototype studio preview](docs/studio-preview.png)

## Prototype

- Two lightweight, procedural 3D upper-body avatars inspired by the supplied
  portraits, with individual cheek/jaw contours, eyes, brows, noses and smiles:
  long, parted black hair and a strapless orange/navy patterned top for host 1;
  shoulder-length, wavy brown hair with lighter strands and a beige patterned
  collared shirt with muted red/green accents for host 2.
- Independent audio-driven mouth opening and vowel-like lip shapes, with a
  noise threshold and smooth transitions. Local spectral/formant analysis approximates
  open A, wide E/I and rounded O/U shapes; it is not phoneme recognition.
- Blinking, breathing, head turns, leaning and speech-dependent arm gestures.
  Arms, hands and fingers stay above the desk surface during these movements
  using a lightweight geometric constraint, without a physics engine.
- A shared desk, studio lighting, microphones and three smoothly changing
  camera angles. Rendering starts at 1280 × 720 with a 30 fps target.
- A diagnostic overlay showing both audio levels and capture status, which can
  be hidden for recording. Rendering continues while the browser has focus.
- A silent demo mode that does not access the microphone or browser audio.

The avatars are initial stylized interpretations, not detailed likenesses.
Their geometry is generated locally; no portrait files or external art assets
are bundled. Clothing patterns are vertex colors on the garment meshes, not
floating ornaments or large textures. Live audio capture currently requires Linux and PipeWire.
The renderer and demo use Bevy's platform-independent APIs; Windows/macOS
builds and audio backends have not been tested yet.

## Build and demo

Requires Rust 1.95 or newer and a Vulkan-capable GPU/driver on Linux. The
initial build downloads and compiles Bevy and can take several minutes.

On Debian 13 with GNOME/Wayland, install the runtime and build tools if missing:

```sh
sudo apt install build-essential libwayland-client0 libxkbcommon0 mesa-vulkan-drivers pipewire-bin pavucontrol obs-studio
cargo run --locked -- --demo
```

For recording, use an optimized build:

```sh
cargo build --release --locked -j 2
./target/release/slofox --demo --clean --auto-camera
```

Wayland libraries are loaded dynamically, so Wayland development packages are
not needed. X11 support is also enabled. All animation is computed locally;
Slofox does not connect to ChatGPT, need an API key, send audio anywhere, or
save audio files.

## Connect Firefox or Chrome and your microphone

1. Create a temporary, dedicated browser output in a separate terminal:

   ```sh
   bash scripts/browser-audio.sh
   ```

   Keep it running. Its playback side forwards browser audio to your current
   default output, so you can still hear ChatGPT. Prefer headphones to keep
   ChatGPT's voice out of the microphone.

2. Play ChatGPT audio in Firefox or Chrome. Open `pavucontrol`, then in
   **Playback** change that browser's output to **Slofox Browser**. This routes
   that browser's playback streams, not just one tab; keep other browser audio
   quiet during recording. The script does not change your default output or
   automatically move any existing streams.

3. List available inputs and outputs:

   ```sh
   ./target/release/slofox --list-devices
   ```

4. Start the studio:

   ```sh
   ./target/release/slofox --browser slofox_browser --microphone auto
   ```

   `auto` uses the default microphone. To select another one, pass its
   `node.name` or `object.serial` from the device list. For host 1, select an
   output sink; Slofox captures its monitor ports. Using the dedicated browser
   sink prevents unrelated desktop sounds from driving that avatar.

5. Speak and play ChatGPT audio separately. Check that only the corresponding
   host's meter and mouth respond. Both channels can also animate simultaneously.
   Press **H** to hide diagnostics once the levels look right.

6. Quit Slofox with **Esc**, then stop the routing script with **Ctrl+C**. The
   temporary sink disappears. If needed, select your normal browser output
   again in `pavucontrol`.

Slofox starts two `pw-record` subprocesses and reads raw mono float audio at
48 kHz in 10 ms blocks. The processes are stopped on normal app exit. Device
selection is done at startup; after unplugging a device or restarting PipeWire,
restart the app. Capture failures appear in diagnostics and the terminal.

## Controls and tuning

| Key | Action |
| --- | --- |
| H | Show/hide the diagnostic overlay |
| 1 | Front camera |
| 2 | Camera from the left |
| 3 | Camera from the right |
| A | Enable/disable automatic camera changes every 14 seconds |
| Esc | Quit |

```sh
./target/release/slofox --help
./target/release/slofox --browser-gain 12 --microphone-gain 6 --threshold 0.012
./target/release/slofox --browser-delay-ms 100 --microphone-delay-ms 50
./target/release/slofox --demo --clean --seconds 12 --screenshot /tmp/slofox.png
```

Gains change mouth sensitivity, not recorded volume. The threshold is a linear
RMS noise gate. Each input has its own bounded, timestamped analysis buffer.
Positive delay settings postpone that host's mouth movement by up to 2000 ms;
use OBS audio offsets when the sound needs delaying instead. The screenshot
option captures only the app window after five seconds; use a longer run when
combining it with `--seconds`.

`--fps` sets the frame pacing target (15–120 fps), not a guaranteed frame
rate. Actual performance depends on the GPU, driver, OBS settings and window
size. The diagnostic panel shows measured fps. Resize the window if a different
resolution is needed.

## Record with OBS on GNOME/Wayland

1. Set OBS's canvas and output to **1280 × 720**, **30 fps**.
2. Add **Screen Capture (PipeWire)** / **Window Capture (PipeWire)**, depending
   on your OBS version, and choose the Slofox window in GNOME's sharing dialog.
3. Add an **Audio Output Capture (PulseAudio)** source selecting **Slofox Browser**,
   plus an **Audio Input Capture (PulseAudio)** source selecting the same
   microphone as Slofox. PipeWire's PulseAudio compatibility service exposes
   these devices. Source labels can differ between OBS versions.
4. Disable any duplicate global Desktop Audio/Mic sources. Disable OBS audio
   monitoring for these sources to avoid echo or feeding audio back into capture.
5. In Advanced Audio Properties, put both voices on the mixed track and,
   optionally, each on its own additional track. Recording to MKV allows
   separate audio tracks; OBS can remux the recording later.
6. Make a short test recording with alternating speech. Adjust Slofox's
   per-input mouth delays or OBS's per-source audio synchronization offsets
   until sound and movement match, then hide the diagnostic overlay.

Slofox outputs video through its window. OBS captures the original audio
directly, so Slofox does not replay or mix the microphone. End-to-end OBS
synchronization must be checked on the recording machine.

## Browser avatar stays silent

Firefox/Chrome may create a new audio stream when a voice session restarts.
Moving the previous stream in `pavucontrol` does not guarantee that the new
stream follows it. If ChatGPT is audible but its Slofox RMS stays at zero,
check the active browser stream's destination again while ChatGPT is speaking.

For automatic routing of one selected tab, first start the browser-audio script
and ChatGPT's voice output, then run `slofox --list-devices`. The `stream` lines
show the exact application name and media title. Start Slofox with that title:

```sh
cargo run --locked -- --route-browser-tab "Your ChatGPT conversation title"
```

The default application is `Firefox`. For Chrome, add `--browser-application`
with its exact application name from the list. While Slofox runs, it checks
once per second and moves matching streams to `--browser`, including newly
created streams. Other tab titles and other applications are left alone. This
is opt-in; ordinary startup does not change any browser routing. If multiple
tabs share the same media title and application, they all match. If the title
changes, restart with the new title. Routing changes use WirePlumber's
`target.object` metadata; `pw-metadata` is included in `pipewire-bin`.

Start the browser-audio script before Slofox and keep it running. If the script
is restarted, restart Slofox too so its capture reconnects to the recreated sink.

## Vowel-like mouth shapes

Formant mode is enabled by default for both hosts. Launch with your usual
audio/routing options; no extra model, API or download is needed. The demo
cycles through open, wide and rounded shapes without capturing audio:

```sh
cargo run --locked -- --demo
```

Audio is low-pass filtered and downsampled from 48 to 12 kHz. Every 10 ms,
the analyzer uses a 32 ms Hamming window. For voiced, harmonically rich audio,
a small FFT compares power in three bands: 150–600 Hz, 600–1400 Hz and
1800–3200 Hz. Relative middle-band energy drives an open shape, relative high-band
energy a wide shape, and a low-frequency-dominated spectrum a rounded shape.
Power ratios rather than absolute level determine colour, so quiet O/U does
not need to produce two separately resolved LPC poles. These broad heuristics
are approximate mouth cues, not measured or recognized individual vowels.

Burg LPC also estimates the first two resonances (F1/F2).
Resonances are extracted from the LPC poles, including close O/U formants that
can merge into a single spectral peak. Periodicity, bandwidth and spectral
strength checks reject uncertain frames; an estimate can be held for at most
50 ms to bridge brief gaps.
FFT does not unconditionally override LPC. A fresh, accepted F1/F2 pair takes
priority when both poles have bandwidth at most 200 Hz, bandwidth/frequency
ratio at most 0.5 and at least 2% relative spectral power. This conservative
quality heuristic preserves clear rounded LPC vowels whose strong F2 can make
the broad FFT middle band look A-like; it is not a calibrated probability.
Otherwise, FFT colour is used when available. With no FFT evidence, accepted
LPC (including the bounded 50 ms hold) remains a fallback. F1 influences jaw
opening and F2 lip width/rounding; if neither is available, volume drives the jaw.
Loudness still
controls movement strength and closes the mouth below `--threshold`. Both lips,
the mouth cavity and teeth move together with frame-rate-independent smoothing.
Audio delays apply to volume, spectral shape and formants together.

The overlay shows the mouth-driving approximate vowel group with `(spectrum)`
or `(LPC)`, or `volume fallback`. The diagnostic log reports raw spectral and
LPC results, frequencies and the LPC reliability flag; they may disagree.
Capture, animation, overlay and the offline example share the same selection
path through `Features::analyzed` and `Features::selected_shape`: raw LPC
estimates remain available from `Analyzer::analyze`, while captured features
contain only LPC evidence eligible for mouth selection. These are approximate
visual cues, not recognized letters. Whispering, high-pitched voices, background
music, consonants and noise can give uncertain or incorrect estimates. This
version does not detect B/P/M lip closures or distinguish individual phonemes.

If one voice consistently looks too open or wide, try a modest increase in
its formant normalization scale (default 1.0; supported range 0.7–1.5):

```sh
cargo run --locked -- --browser-formant-scale 1.15 --microphone-formant-scale 1.0
```

The scales normalize spectral band frequencies and divide measured F1/F2 before
mapping; they do not alter pitch, sound or raw diagnostic frequencies.
Tune with sustained A, E/I and O/U sounds.
The previous animation remains available with `--mouth-mode volume`.

If O/U is consistently classified as E/I, add `--audio-diagnostics` to your
usual live command. Once per second, stderr prints a `Slofox Microphone` and
`Slofox Browser` line with RMS, analysis status and resonance candidates:
`850Hz/BW150Hz/0.0010` means an 850 Hz resonance, a 150 Hz bandwidth and power
relative to the strongest LPC spectral bin in the 150–3000 Hz range.
The list also includes candidates
rejected for excessive bandwidth or weak power. No samples are printed or
saved, and normal startup does not emit these diagnostics. In demo mode no
capture runs, so this option prints no capture diagnostics.

For troubleshooting, sustain each of A, I, O and U for several seconds and
compare the corresponding microphone lines. A weak eligible F2 now causes a
bounded volume fallback rather than promoting a stronger F3 to F2. Candidates
with 500–900 Hz bandwidth are admitted only if their relative power is at least
2%; otherwise the usual 30–500 Hz bandwidth criterion applies. The selected
F2 must have at least 1% relative power. This preserves strong broad O/U
resonances while avoiding confident E/I labels from very weak high resonances.
If F2 is genuinely missing or too weak, the LPC result is rejected; spectral
colour can still animate the mouth without claiming a measured F2.
These checks cannot reconstruct it. Diagnostics help distinguish that case
from an incorrect mouth mapping. Gain controls
mouth strength, not the measured formant frequencies.

The general formant-to-mouth approach is described by
[Ishi et al., Interspeech 2012](https://www.isca-archive.org/interspeech_2012/ishi12_interspeech.html).
Slofox uses its own simplified mapping, not the paper's complete method.
For background on Burg LPC and formant extraction, see the
[Praat documentation](https://fon.hum.uva.nl/praat/manual/Sound__To_Formant__burg____.html).

## Development

To inspect a recording locally without playing it, an optional `ffmpeg`
conversion can feed the offline analysis example:

```sh
ffmpeg -v error -i /path/to/vowels.wav -ac 1 -ar 48000 -f f32le /tmp/vowels.f32
cargo run --locked --example analyze_audio -- /tmp/vowels.f32
```

`tests/private_vowels.rs` is an opt-in regression for the private 25-second
A/I/O/U/loud-O reference. It uses stable intervals 0.5–3.5, 5.5–8.5,
10.5–13.5, 16–18.5 and 20.5–23 seconds and checks both classification and
animated mouth coefficients at original and half amplitude:

```sh
SLOFOX_VOWELS_F32=/tmp/vowels.f32 cargo test --locked --test private_vowels -- --ignored --nocapture
```

The reference recording is deliberately not distributed or committed. Ordinary
tests do not open audio files; synthetic spectral, normalization, silence and
delay tests remain self-contained. The synthetic 350/850 Hz, 140 Hz pitch
regression explicitly checks conflicting raw FFT evidence, selected source,
and smoothed mouth coefficients; a wider A/I/O matrix covers four pitches and
two amplitudes. Passing one voice reference does not establish
accuracy for other voices or ordinary conversational phonemes.

```sh
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
```

An optional integration test exercises real PipeWire sink capture and channel
separation without opening the microphone or playing sound on your speakers:

```sh
cargo test --locked --test pipewire -- --ignored
```

It requires a running PipeWire session and creates two temporary null sinks
and virtual browser/microphone nodes. It tests both sink-monitor and source capture,
including synthetic vowel features and separation of the two inputs.
It is skipped in the normal test suite.

`src/audio.rs` separates capture, timestamped feature buffering, RMS analysis
and the smoothed `SpeechPose`. `src/formants.rs` implements local resonance
analysis and the vowel-like mouth mapping without additional dependencies.
`src/studio.rs` builds the replaceable avatar geometry and stage;
`src/main.rs` connects analysis to animation, cameras and diagnostics.

Future steps include rigged glTF models with mouth blend shapes, more accurate
phoneme-derived visemes, more varied gestures, GUI device selection and native audio backends
for other operating systems.

Reference documentation: [Bevy](https://bevy.org/),
[PipeWire loopback](https://docs.pipewire.org/page_module_loopback.html),
[pw-cat/pw-record](https://docs.pipewire.org/page_man_pw-cat_1.html),
[OBS window capture](https://obsproject.com/kb/window-capture-sources).

## License

This project is dual-licensed under the [MIT License](LICENSE-MIT) or the
[Apache License, Version 2.0](LICENSE-APACHE), at your option.

SPDX license expression: `MIT OR Apache-2.0`.
