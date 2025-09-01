import React, { useState } from 'react';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '../ui/dialog';
import { Button } from '../ui/button';
import { X } from 'lucide-react';
import LoginForm from './LoginForm';
import SignUpForm from './SignUpForm';

interface AuthModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSuccess?: () => void;
  redirectTo?: string;
  initialMode?: 'login' | 'signup';
}

const AuthModal: React.FC<AuthModalProps> = ({
  isOpen,
  onClose,
  onSuccess,
  redirectTo = '/command-center',
  initialMode = 'login',
}) => {
  const [mode, setMode] = useState<'login' | 'signup'>(initialMode);

  const handleSuccess = () => {
    onClose();
    onSuccess?.();
  };

  const handleSwitchToSignUp = () => {
    setMode('signup');
  };

  const handleSwitchToLogin = () => {
    setMode('login');
  };

  return (
    <Dialog open={isOpen} onOpenChange={onClose}>
      <DialogContent
        className="modal-content-glass app-overlay context-modal max-w-md w-full mx-4 border-roman-bronze overflow-hidden p-0"
        showCloseButton={false}
      >
        <Button
          onClick={onClose}
          variant="ghost"
          size="icon"
          className="absolute top-4 right-4 w-6 h-6 rounded-full bg-roman-crimson/90 border border-roman-crimson/40 text-primary-foreground hover:bg-roman-crimson transition-all duration-200 hover:scale-105 z-10"
        >
          <X className="h-3 w-3" />
        </Button>

        <div className="p-6">
          <DialogHeader className="text-center mb-6">
            <DialogTitle className="text-2xl font-bold text-roman-gold">
              {mode === 'login' ? 'Sign In' : 'Join the Legion'}
            </DialogTitle>
            <DialogDescription className="text-text-secondary">
              {mode === 'login'
                ? 'Enter your credentials to access the Command Center.'
                : 'Create your Testudo Trading account to get started.'}
            </DialogDescription>
          </DialogHeader>

          {mode === 'login' ? (
            <LoginForm
              onSuccess={handleSuccess}
              onSwitchToSignUp={handleSwitchToSignUp}
              redirectTo={redirectTo}
            />
          ) : (
            <SignUpForm
              onSuccess={handleSuccess}
              onSwitchToLogin={handleSwitchToLogin}
              redirectTo={redirectTo}
            />
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
};

export default AuthModal;
