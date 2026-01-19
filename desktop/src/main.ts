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
let audioData: number[] = new Array(20).fill(0);
let animationId: number | null = null;
let previewAudioTimer: number | null = null;

function startVisualizer() {
  const canvas = document.getElementById("audio-canvas") as HTMLCanvasElement;
  if (!canvas) return;

  const ctx = canvas.getContext("2d", { alpha: true });
  if (!ctx) return;

  // Handle high DPI displays with device-pixel alignment for crisp bars
  const dpr = window.devicePixelRatio || 1;
  const rect = canvas.getBoundingClientRect();
  const pixelWidth = Math.max(1, Math.round(rect.width * dpr));
  const pixelHeight = Math.max(1, Math.round(rect.height * dpr));
  const gap = Math.max(1, Math.round(2 * dpr));
  const minBarHeight = Math.max(1, Math.round(4 * dpr));

  canvas.width = pixelWidth;
  canvas.height = pixelHeight;
  ctx.setTransform(1, 0, 0, 1, 0, 0);

  const draw = () => {
    const width = pixelWidth;
    const height = pixelHeight;
    const centerY = Math.round(height / 2);

    ctx.clearRect(0, 0, width, height);

    // Draw bars as filled rectangles for crisp rendering
    const numBars = audioData.length;
    const barWidth = Math.max(
      1,
      Math.floor((width - gap * (numBars + 1)) / numBars),
    );
    const totalWidth = barWidth * numBars + gap * (numBars - 1);
    const startX = Math.round((width - totalWidth) / 2);

    for (let i = 0; i < numBars; i++) {
      const x = Math.round(startX + i * (barWidth + gap));
      const val = Math.min(1.0, audioData[i] * 4.0);
      const barHeight = Math.max(minBarHeight, Math.round(val * (height * 0.5)));
      const y = Math.round(centerY - barHeight / 2);

      ctx.fillStyle = `rgba(255, 255, 255, ${0.6 + val * 0.4})`;
      ctx.fillRect(x, y, barWidth, barHeight);
    }

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
    <span class="status-text">Thinking...</span>
  `;
  return popup;
}

// Create success UI
function createSuccessUI(): HTMLElement {
  const popup = document.createElement("div");
  popup.className = "popup";
  popup.innerHTML = `
    <span class="status-text success">
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"></polyline></svg>
      Copied
    </span>
  `;
  return popup;
}

// Create error UI
function createErrorUI(message: string): HTMLElement {
  const popup = document.createElement("div");
  popup.className = "popup";
  popup.innerHTML = `
    <span class="status-text error">
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"></circle><line x1="12" y1="8" x2="12" y2="12"></line><line x1="12" y1="16" x2="12.01" y2="16"></line></svg>
      ${message}
    </span>
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
  let lastVal = 0;
  listen<number>("audio-level", (event) => {
    // Boost low signals and apply smoothing
    const targetVal = Math.sqrt(event.payload);

    // Simple smoothing (Lerp) to make it feel more "liquid"
    const smoothedVal = lastVal + (targetVal - lastVal) * 0.4;
    lastVal = smoothedVal;

    // Push to buffer, remove old
    audioData.push(smoothedVal);
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
