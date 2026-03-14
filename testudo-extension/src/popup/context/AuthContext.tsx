import { createSignal, createContext, useContext, onMount, type JSX } from "solid-js";
import browser from "webextension-polyfill";

export interface AuthState {
  authenticated: () => boolean;
  email: () => string;
  login: (email: string, password: string) => Promise<{ success: boolean; error?: string }>;
  register: (email: string, password: string) => Promise<{ success: boolean; error?: string }>;
  logout: () => Promise<void>;
  checkAuth: () => Promise<void>;
}

const AuthContext = createContext<AuthState>();

export function AuthProvider(props: { children: JSX.Element; onReady: (authed: boolean) => void }) {
  const [authenticated, setAuthenticated] = createSignal(false);
  const [email, setEmail] = createSignal("");

  async function checkAuth() {
    try {
      const response = await browser.runtime.sendMessage({ type: "AUTH_STATUS" }) as {
        authenticated: boolean;
        email?: string;
      };
      setAuthenticated(response.authenticated);
      if (response.email) setEmail(response.email);
      props.onReady(response.authenticated);
    } catch (err) {
      console.error("Auth check failed:", err);
      setAuthenticated(false);
      props.onReady(false);
    }
  }

  async function login(loginEmail: string, password: string): Promise<{ success: boolean; error?: string }> {
    const response = await browser.runtime.sendMessage({
      type: "LOGIN",
      email: loginEmail.trim(),
      password,
    });

    if (!response || typeof response !== "object") {
      return { success: false, error: "No response from service worker — check chrome://extensions for errors" };
    }

    const result = response as { success: boolean; error?: string };
    if (result.success) {
      await checkAuth();
    }
    return result;
  }

  async function register(regEmail: string, password: string): Promise<{ success: boolean; error?: string }> {
    const response = await browser.runtime.sendMessage({
      type: "REGISTER",
      email: regEmail.trim(),
      password,
    });

    if (!response || typeof response !== "object") {
      return { success: false, error: "No response from service worker — check chrome://extensions for errors" };
    }

    const result = response as { success: boolean; error?: string };
    if (result.success) {
      await checkAuth();
    }
    return result;
  }

  async function logout() {
    await browser.runtime.sendMessage({ type: "LOGOUT" });
    setAuthenticated(false);
    setEmail("");
  }

  onMount(checkAuth);

  const state: AuthState = {
    authenticated,
    email,
    login,
    register,
    logout,
    checkAuth,
  };

  return (
    <AuthContext.Provider value={state}>
      {props.children}
    </AuthContext.Provider>
  );
}

export function useAuth(): AuthState {
  const ctx = useContext(AuthContext);
  if (!ctx) throw new Error("useAuth must be used within AuthProvider");
  return ctx;
}
