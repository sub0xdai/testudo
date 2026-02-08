import browser from "webextension-polyfill";

type ExecutionMode = "paper" | "live";

const backendUrlInput = document.getElementById("backend-url") as HTMLInputElement;
const toggleBtns = document.querySelectorAll<HTMLElement>(".toggle-btn");
const saveStatus = document.getElementById("save-status") as HTMLElement;

let currentMode: ExecutionMode = "paper";

async function loadSettings(): Promise<void> {
  const stored = await browser.storage.local.get(["backendUrl", "executionMode"]);
  backendUrlInput.value = (stored.backendUrl as string) || "http://localhost:8080";
  currentMode = (stored.executionMode as ExecutionMode) || "paper";
  updateToggleUI();
}

function updateToggleUI(): void {
  toggleBtns.forEach((btn) => {
    const mode = btn.dataset.mode as ExecutionMode;
    btn.classList.toggle("active", mode === currentMode);
    if (mode === "live") {
      btn.classList.toggle("live", mode === currentMode);
    }
  });
}

async function saveSettings(): Promise<void> {
  await browser.storage.local.set({
    backendUrl: backendUrlInput.value.trim(),
    executionMode: currentMode,
  });
  saveStatus.classList.add("visible");
  setTimeout(() => saveStatus.classList.remove("visible"), 1500);
}

backendUrlInput.addEventListener("change", saveSettings);

toggleBtns.forEach((btn) => {
  btn.addEventListener("click", () => {
    currentMode = btn.dataset.mode as ExecutionMode;
    updateToggleUI();
    saveSettings();
  });
});

loadSettings();
