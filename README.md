# Windows Whisper 🎙️

A lightweight, push-to-talk speech-to-text application for Windows. Inspired by Mac Whisper, this tool allows you to record audio with a global hotkey and get near-instant transcription pasted directly into your active application.

Powered by [Cloudflare Workers AI](https://developers.cloudflare.com/workers-ai/) (@cf/openai/whisper) for fast and accurate transcription.

## Features

- **Global Hotkey:** Press and hold a configurable hotkey to record specific snippets of audio.
- **Fast Transcription:** Utilizes Cloudflare's edge network running OpenAI's Whisper model for rapid speech-to-text.
- **Auto Paste:** Automatically types the transcribed text into the currently active text field.
- **Clipboard Sync:** Copies the transcription to your clipboard for backup.
- **Minimal UI:** Unobtrusive popup near your cursor showing recording status and audio visualization.
- **Privacy Focused:** Your audio is processed on your own Cloudflare Worker instance.

---

## Architecture

The project consists of two main components:

1.  **Desktop App:** A minimal Windows application built with [Tauri v2](https://v2.tauri.app/) (Rust + TypeScript). It handles audio recording, global shortcuts, and OS integration (simulating keystrokes).
2.  **Worker:** A Cloudflare Worker that acts as the backend API. It receives the audio data and runs the Whisper AI model.

---

## Prerequisites

Before getting started, ensure you have the following installed:

- **Node.js** (v18 or later)
- **Rust** (latest stable)
- **Cloudflare Account** (for Workers AI)

## Setup Instructions

### 1. Backend: Cloudflare Worker

**Quick Start (PowerShell):**
You can run the included helper script to automate the setup:

```powershell
./deploy-worker.ps1
```

**Manual Setup:**
If you prefer to set it up manually:

1.  Navigate to the `worker` directory:

    ```bash
    cd worker
    ```

2.  Install dependencies:

    ```bash
    npm install
    ```

3.  Authenticate with Cloudflare (if you haven't already):

    ```bash
    npx wrangler login
    ```

4.  Deploy the worker:

    ```bash
    npm run deploy
    ```

5.  **Important:** Note down the worker URL provided after deployment (e.g., `https://windows-whisper.your-name.workers.dev`).

6.  Set a secure API Key for your worker (this protects your endpoint):
    ```bash
    npx wrangler secret put API_KEY
    ```
    _Enter a strong random string when prompted._

### 2. Frontend: Desktop Application

Now, build and run the Windows desktop application.

1.  Navigate to the `desktop` directory:

    ```bash
    cd ../desktop
    ```

2.  Install dependencies:

    ```bash
    npm install
    ```

3.  **Configuration:**

    - Launch the app in development mode first to configure it.
    - Run: `npm run tauri dev`
    - Once the app is running, right-click the system tray icon (or use the settings menu if available) to open **Settings**.
    - Enter your **Worker URL** (from step 1.5) and **API Key** (from step 1.6).
    - Save the settings.

4.  **Build for Production:**
    To create a standalone `.exe` installer:
    ```bash
    npm run tauri build
    ```
    The output installer will be located in `src-tauri/target/release/bundle/msi/` or `nsis/`.

---

## Usage

1.  **Start the Application:** Run the installed `Windows Whisper` app. It will run in the background.
2.  **Record:** **Press and Hold** the configured hotkey (Default: `F13` or whatever is set in settings).
    - _Note: If you don't have an F13 key, check the settings to rebind it._
3.  **Speak:** Speak clearly into your microphone while holding the key. You'll see a small visualizer near your cursor.
4.  **Release:** Release the key to stop recording.
5.  **Transcribe:** Wait a moment for the "Processing" indicator.
6.  **Done:** The text will be automatically pasted into your active window!

## Troubleshooting

- **"Unauthorized" Error:** Ensure the API Key in the desktop settings matches the one set in your Cloudflare Worker secrets.
- **Audio not recording:** Check your Windows sound settings and ensure the default microphone is selected.
- **Nothing types out:** Some applications block simulated keystrokes. Try pasting manually (`Ctrl+V`) as the text is also copied to the clipboard.

## Development

- **Worker:** sending test requests:
  ```bash
  curl -X POST https://your-worker.workers.dev/transcribe \
    -H "X-API-Key: YOUR_KEY" \
    -H "Content-Type: application/json" \
    -d '{"audio": "BASE64_ENCODED_AUDIO_STRING"}'
  ```

---

Built with ❤️ using Tauri and Cloudflare Workers.
