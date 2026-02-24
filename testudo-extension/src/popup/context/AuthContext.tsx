import { createSignal, createContext, useContext, onMount, type JSX } from "solid-js";
import browser from "webextension-polyfill";

export interface AuthState {
  authenticated: () => boolean;
  email: () => string;
  paperOnly: () => boolean;
  login: (email: string, password: string) => Promise<{ success: boolean; error?: string }>;
  register: (email: string, password: string) => Promise<{ success: boolean; error?: string }>;
  logout: () => Promise<void>;
  continueWithoutAccount: () => Promise<void>;
  checkAuth: () => Promise<void>;
}

const AuthContext = createContext<AuthState>();

export function AuthProvider(props: { children: JSX.Element; onReady: (authed: boolean, paperOnly: boolean) => void }) {
  const [authenticated, setAuthenticated] = createSignal(false);
  const [email, setEmail] = createSignal("");
  const [paperOnly, setPaperOnly] = createSignal(false);

  async function checkAuth() {
    const stored = await browser.storage.local.get(["paperOnly"]);
    if (stored.paperOnly) {
      setPaperOnly(true);
      setAuthenticated(false);
      props.onReady(false, true);
      return;
    }

    const response = await browser.runtime.sendMessage({ type: "AUTH_STATUS" }) as {
      authenticated: boolean;
      email?: string;
    };
    setAuthenticated(response.authenticated);
    if (response.email) setEmail(response.email);
    props.onReady(response.authenticated, false);
  }

  async function login(loginEmail: string, password: string): Promise<{ success: boolean; error?: string }> {
    const response = await browser.runtime.sendMessage({
      type: "LOGIN",
      email: loginEmail.trim(),
      password,
    }) as { success: boolean; error?: string };

    if (response.success) {
      await browser.storage.local.remove("paperOnly");
      setPaperOnly(false);
      await checkAuth();
    }
    return response;
  }

  async function register(regEmail: string, password: string): Promise<{ success: boolean; error?: string }> {
    const response = await browser.runtime.sendMessage({
      type: "REGISTER",
      email: regEmail.trim(),
      password,
    }) as { success: boolean; error?: string };

    if (response.success) {
      await browser.storage.local.remove("paperOnly");
      setPaperOnly(false);
      await checkAuth();
    }
    return response;
  }

  async function logout() {
    await browser.runtime.sendMessage({ type: "LOGOUT" });
    await browser.storage.local.remove("paperOnly");
    setAuthenticated(false);
    setEmail("");
    setPaperOnly(false);
  }

  async function continueWithoutAccount() {
    await browser.storage.local.set({ paperOnly: true, executionMode: "paper" });
    setPaperOnly(true);
    setAuthenticated(false);
  }

  onMount(checkAuth);

  const state: AuthState = {
    authenticated,
    email,
    paperOnly,
    login,
    register,
    logout,
    continueWithoutAccount,
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
