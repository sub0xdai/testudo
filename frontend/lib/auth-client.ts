import { createAuthClient } from 'better-auth/react';

export const authClient = createAuthClient({
  baseURL: import.meta.env.VITE_API_URL || 'http://localhost:3000',
  basePath: '/api/auth',
});

// Export hooks for easy use in components
export const {
  useSession,
  signIn,
  signUp,
  signOut,
  useListSessions,
  getSession,
} = authClient;

// Custom hooks for trading platform specific functionality
export const useUser = () => {
  const session = useSession();
  return {
    user: session.data?.user,
    isAuthenticated: !!session.data?.user,
    isLoading: session.isPending,
    error: session.error,
  };
};

export const useTradingAuth = () => {
  const { user, isAuthenticated, isLoading } = useUser();

  return {
    user,
    isAuthenticated,
    isLoading,
    canTrade:
      isAuthenticated &&
      user?.emailVerified &&
      user?.isVerified &&
      user?.twoFactorEnabled,
    requiresEmailVerification: isAuthenticated && !user?.emailVerified,
    requiresAccountVerification: isAuthenticated && !user?.isVerified,
    requiresTwoFactor: isAuthenticated && !user?.twoFactorEnabled,
  };
};
