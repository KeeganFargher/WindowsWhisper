import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./styles.css";

type AppState = "idle" | "recording" | "processing" | "success" | "error";

// Create the popup UI
// Create the popup UI
function createPopupUI(): HTMLElement {
    const popup = document.createElement("div");
    popup.className = "popup";
    popup.innerHTML = `
    <div class="audio-visualizer">
        <canvas id="audio-canvas" width="300" height="100"></canvas>
    </div>
  `;
    return popup;
}

// Visualizer State
let audioData: number[] = new Array(50).fill(0);
let animationId: number | null = null;

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
        ctx.strokeStyle = "#e94560"; // var(--accent)
        ctx.shadowBlur = 10;
        ctx.shadowColor = "rgba(233, 69, 96, 0.5)";

        // Draw mirrored waveform
        ctx.beginPath();

        const barWidth = width / audioData.length;

        for (let i = 0; i < audioData.length; i++) {
            const x = i * barWidth;
            // amplify the signal for better visuals
            const val = Math.min(1.0, audioData[i] * 2.5);
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
    switch (state) {
        case "recording":
            app.appendChild(createPopupUI());
            startVisualizer();
            break;
        case "processing":
            stopVisualizer();
            app.appendChild(createProcessingUI());
            break;
        case "success":
            stopVisualizer();
            app.appendChild(createSuccessUI());
            // Auto-hide after 1.5 seconds
            setTimeout(() => {
                invoke("hide_popup");
            }, 1500);
            break;
        case "error":
            stopVisualizer();
            app.appendChild(createErrorUI(data || "Unknown error"));
            // Auto-hide after 2 seconds
            setTimeout(() => {
                invoke("hide_popup");
            }, 2000);
            break;
        case "idle":
        default:
            stopVisualizer();
            // Empty state
            break;
    }
}

// Initialize the app
async function init() {
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
