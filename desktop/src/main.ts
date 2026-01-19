import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./styles.css";

type AppState = "idle" | "recording" | "processing" | "success" | "error";

const win = window as Window & {
  __TAURI__?: unknown;
  __TAURI_INTERNALS__?: unknown;
};
const tauriAvailable =
  typeof win.__TAURI__ !== "undefined" ||
  typeof win.__TAURI_INTERNALS__ !== "undefined";
const previewMode =
  !tauriAvailable || new URLSearchParams(window.location.search).has("preview");

// Create the popup UI
// Create the popup UI
function createPopupUI(): HTMLElement {
  const popup = document.createElement("div");
  popup.className = "popup";
  popup.innerHTML = `
    <div class="audio-visualizer">
        <canvas id="audio-canvas" width="300" height="150"></canvas>
    </div>
  `;
  return popup;
}

// Visualizer State
let audioData: number[] = new Array(50).fill(0);
let animationId: number | null = null;
let previewAudioTimer: number | null = null;

function startVisualizer() {
  const canvas = document.getElementById("audio-canvas") as HTMLCanvasElement;
  if (!canvas) return;

  const ctx = canvas.getContext("2d");
  if (!ctx) return;

  // Handle high output density displays
  const dpr = window.devicePixelRatio || 1;
  const rect = canvas.getBoundingClientRect();
  canvas.width = rect.width * dpr;
  canvas.height = rect.height * dpr;
  ctx.scale(dpr, dpr);

  const draw = () => {
    const width = rect.width;
    const height = rect.height;
    const centerY = height / 2;

    ctx.clearRect(0, 0, width, height);

    // Style
    ctx.lineWidth = 2;
    ctx.lineCap = "round";
    ctx.strokeStyle = "#ffffff";
    ctx.shadowBlur = 8;
    ctx.shadowColor = "rgba(255, 255, 255, 0.4)";

    // Draw mirrored waveform
    ctx.beginPath();

    const barWidth = width / audioData.length;

    for (let i = 0; i < audioData.length; i++) {
      const x = i * barWidth;
      // amplify the signal for better visuals
      const val = Math.min(1.0, audioData[i] * 5);
      const barHeight = val * (height * 0.8);

      // Draw a vertical line centered vertically
      ctx.moveTo(x + barWidth / 2, centerY - barHeight / 2);
      ctx.lineTo(x + barWidth / 2, centerY + barHeight / 2);
    }
    ctx.stroke();

    animationId = requestAnimationFrame(draw);
  };

  draw();
}

function stopVisualizer() {
  if (animationId) {
    cancelAnimationFrame(animationId);
    animationId = null;
  }
  // Reset audio data when stopping visualizer
  audioData.fill(0);
}

function startPreviewAudio() {
  if (!previewMode || previewAudioTimer !== null) return;
  previewAudioTimer = window.setInterval(() => {
    audioData.push(Math.random());
    audioData.shift();
  }, 50);
}

function stopPreviewAudio() {
  if (previewAudioTimer === null) return;
  window.clearInterval(previewAudioTimer);
  previewAudioTimer = null;
}

// Create processing UI
function createProcessingUI(): HTMLElement {
  const popup = document.createElement("div");
  popup.className = "popup";
  popup.innerHTML = `
    <div class="spinner"></div>
  `;
  return popup;
}

// Create success UI
function createSuccessUI(): HTMLElement {
  const popup = document.createElement("div");
  popup.className = "popup";
  popup.innerHTML = `
    <span class="status-text success">✓ Copied!</span>
  `;
  return popup;
}

// Create error UI
function createErrorUI(message: string): HTMLElement {
  const popup = document.createElement("div");
  popup.className = "popup";
  popup.innerHTML = `
    <span class="status-text error">✗ ${message}</span>
  `;
  return popup;
}

// Update the app UI based on state
function updateUI(state: AppState, data?: string) {
  const app = document.getElementById("app")!;
  app.innerHTML = "";
  stopPreviewAudio();
  switch (state) {
    case "recording":
      app.appendChild(createPopupUI());
      startVisualizer();
      startPreviewAudio();
      break;
    case "processing":
      stopVisualizer();
      app.appendChild(createProcessingUI());
      break;
    case "success":
      stopVisualizer();
      app.appendChild(createSuccessUI());
      // Auto-hide after 1.5 seconds
      if (tauriAvailable && !previewMode) {
        setTimeout(() => {
          void invoke("hide_popup");
        }, 1500);
      }
      break;
    case "error":
      stopVisualizer();
      app.appendChild(createErrorUI(data || "Unknown error"));
      // Auto-hide after 2 seconds
      if (tauriAvailable && !previewMode) {
        setTimeout(() => {
          void invoke("hide_popup");
        }, 2000);
      }
      break;
    case "idle":
    default:
      stopVisualizer();
      // Empty state
      break;
  }
}

function setupPreviewMode() {
  document.body.classList.add("preview");
  document.documentElement.style.setProperty("--popup-width", "120px");
  document.documentElement.style.setProperty("--popup-height", "45px");
  document.documentElement.style.setProperty("--preview-scale", "3");

  const panel = document.createElement("div");
  panel.className = "preview-panel";
  panel.innerHTML = `
    <div class="preview-title">Preview</div>
    <div class="preview-buttons">
        <button type="button" data-state="recording">Recording</button>
        <button type="button" data-state="processing">Processing</button>
        <button type="button" data-state="success">Success</button>
        <button type="button" data-state="error">Error</button>
        <button type="button" data-state="idle">Idle</button>
    </div>
    <label class="preview-scale">
        Scale
        <input id="preview-scale" type="range" min="1" max="6" step="0.1" value="3" />
    </label>
  `;
  panel.addEventListener("click", (event) => {
    const target = event.target as HTMLElement;
    const button = target.closest("button");
    if (!button) return;
    const state = button.getAttribute("data-state") as AppState | null;
    if (!state) return;
    if (state === "error") {
      updateUI(state, "Something went wrong");
      return;
    }
    updateUI(state, "Copied!");
  });
  const scaleInput = panel.querySelector<HTMLInputElement>("#preview-scale");
  scaleInput?.addEventListener("input", () => {
    document.documentElement.style.setProperty(
      "--preview-scale",
      scaleInput.value,
    );
  });

  document.body.appendChild(panel);
  updateUI("recording");
}

// Initialize the app
async function init() {
  if (previewMode) {
    setupPreviewMode();
    console.log("Windows Whisper preview mode initialized");
    return;
  }
  // Listen for audio levels
  listen<number>("audio-level", (event) => {
    // Boost low signals with sqrt
    const val = Math.sqrt(event.payload);

    // Push to buffer, remove old
    audioData.push(val);
    audioData.shift();
  });

  listen("show-idle", () => {
    updateUI("idle");
  });

  // Listen for state changes from Rust backend
  listen<{ state: AppState; data?: string }>("state-change", (event) => {
    updateUI(event.payload.state, event.payload.data);
  });

  // Listen for window focus/show events
  listen("show-recording", () => {
    updateUI("recording");
  });

  listen("show-processing", () => {
    updateUI("processing");
  });

  listen<string>("show-success", (event) => {
    updateUI("success", event.payload);
  });

  listen<string>("show-error", (event) => {
    updateUI("error", event.payload);
  });

  // Prevent context menu
  document.addEventListener("contextmenu", (e) => e.preventDefault());

  // Start with idle state
  updateUI("idle");

  console.log("Windows Whisper initialized");
}

init();
