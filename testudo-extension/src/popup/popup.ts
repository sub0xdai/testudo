import browser from "webextension-polyfill";

type ExecutionMode = "paper" | "live";

const backendUrlInput = document.getElementById("backend-url") as HTMLInputElement;
const toggleBtns = document.querySelectorAll<HTMLElement>(".toggle-btn");
const saveStatus = document.getElementById("save-status") as HTMLElement;

// Auth elements
const authLoggedOut = document.getElementById("auth-logged-out") as HTMLElement;
const authLoggedIn = document.getElementById("auth-logged-in") as HTMLElement;
const loginEmail = document.getElementById("login-email") as HTMLInputElement;
const loginPassword = document.getElementById("login-password") as HTMLInputElement;
const loginBtn = document.getElementById("login-btn") as HTMLButtonElement;
const loginError = document.getElementById("login-error") as HTMLElement;
const authEmail = document.getElementById("auth-email") as HTMLElement;
const logoutBtn = document.getElementById("logout-btn") as HTMLButtonElement;

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

// --- Auth UI (EXT-05 FR-2) ---

async function checkAuthStatus(): Promise<void> {
  const response = await browser.runtime.sendMessage({ type: "AUTH_STATUS" }) as {
    authenticated: boolean;
    email?: string;
  };

  if (response.authenticated) {
    authLoggedOut.classList.add("hidden");
    authLoggedIn.classList.remove("hidden");
    authEmail.textContent = response.email || "authenticated";
  } else {
    authLoggedOut.classList.remove("hidden");
    authLoggedIn.classList.add("hidden");
  }
}

loginBtn.addEventListener("click", async () => {
  loginError.classList.remove("visible");
  loginBtn.textContent = "Logging in...";
  loginBtn.disabled = true;

  const response = await browser.runtime.sendMessage({
    type: "LOGIN",
    email: loginEmail.value.trim(),
    password: loginPassword.value,
  }) as { success: boolean; error?: string };

  loginBtn.textContent = "Login";
  loginBtn.disabled = false;

  if (response.success) {
    loginPassword.value = "";
    loginError.classList.remove("visible");
    checkAuthStatus();
  } else {
    loginError.textContent = response.error || "Login failed";
    loginError.classList.add("visible");
  }
});

// Submit on Enter in password field
loginPassword.addEventListener("keydown", (e) => {
  if (e.key === "Enter") {
    loginBtn.click();
  }
});

logoutBtn.addEventListener("click", async () => {
  await browser.runtime.sendMessage({ type: "LOGOUT" });
  checkAuthStatus();
});

// --- Initialize ---

loadSettings();
checkAuthStatus();
