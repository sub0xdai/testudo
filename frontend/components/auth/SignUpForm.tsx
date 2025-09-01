import React, { useState } from 'react';
import { signUp } from '../../lib/auth-client';
import { Button } from '../ui/button';
import { Input } from '../ui/input';
import { Label } from '../ui/label';
import { Checkbox } from '../ui/checkbox';
import { Card } from '../ui/card';
import { AlertCircle } from 'lucide-react';

interface SignUpFormProps {
  onSuccess?: () => void;
  onSwitchToLogin?: () => void;
  redirectTo?: string;
}

const SignUpForm: React.FC<SignUpFormProps> = ({
  onSuccess,
  onSwitchToLogin,
  redirectTo = '/command-center',
}) => {
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [name, setName] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setIsLoading(true);
    setError(null);

    // Validate passwords match
    if (password !== confirmPassword) {
      setError('Passwords do not match');
      setIsLoading(false);
      return;
    }

    // Validate password strength
    if (password.length < 8) {
      setError('Password must be at least 8 characters long');
      setIsLoading(false);
      return;
    }

    try {
      const result = await signUp.email({
        email,
        password,
        name,
        callbackURL: redirectTo,
      });

      if (result.error) {
        setError(result.error.message || 'Sign up failed');
      } else {
        // Sign up successful
        onSuccess?.();
      }
    } catch (err) {
      setError('An unexpected error occurred');
      console.error('Sign up error:', err);
    } finally {
      setIsLoading(false);
    }
  };

  const [agreedToTerms, setAgreedToTerms] = useState(false);

  return (
    <div className="w-full">
      <div className="mb-6 text-center">
        <p className="text-muted-foreground">
          Create your Testudo Trading account
        </p>
      </div>

      {error && (
        <Card className="mb-4 p-4 border-destructive/50 bg-destructive/10 rounded-lg">
          <div className="flex items-center space-x-2 text-destructive">
            <AlertCircle className="h-4 w-4" />
            <span className="text-sm">{error}</span>
          </div>
        </Card>
      )}

      <form onSubmit={handleSubmit} className="space-y-4">
        <div className="space-y-2">
          <Label htmlFor="name" className="text-foreground font-medium">
            Full Name
          </Label>
          <Input
            id="name"
            type="text"
            value={name}
            onChange={e => setName(e.target.value)}
            className="bg-background border-border focus:border-roman-bronze focus:ring-roman-bronze/20"
            placeholder="Enter your full name"
            required
            disabled={isLoading}
          />
        </div>

        <div className="space-y-2">
          <Label htmlFor="signup-email" className="text-foreground font-medium">
            Email
          </Label>
          <Input
            id="signup-email"
            type="email"
            value={email}
            onChange={e => setEmail(e.target.value)}
            className="bg-background border-border focus:border-roman-bronze focus:ring-roman-bronze/20"
            placeholder="Enter your email"
            required
            disabled={isLoading}
          />
        </div>

        <div className="space-y-2">
          <Label
            htmlFor="signup-password"
            className="text-foreground font-medium"
          >
            Password
          </Label>
          <Input
            id="signup-password"
            type="password"
            value={password}
            onChange={e => setPassword(e.target.value)}
            className="bg-background border-border focus:border-roman-bronze focus:ring-roman-bronze/20"
            placeholder="Create a strong password"
            required
            disabled={isLoading}
            minLength={8}
          />
          <p className="text-xs text-muted-foreground">
            Must be at least 8 characters long
          </p>
        </div>

        <div className="space-y-2">
          <Label
            htmlFor="confirm-password"
            className="text-foreground font-medium"
          >
            Confirm Password
          </Label>
          <Input
            id="confirm-password"
            type="password"
            value={confirmPassword}
            onChange={e => setConfirmPassword(e.target.value)}
            className="bg-background border-border focus:border-roman-bronze focus:ring-roman-bronze/20"
            placeholder="Confirm your password"
            required
            disabled={isLoading}
          />
        </div>

        <div className="flex items-center space-x-2">
          <Checkbox
            id="terms"
            checked={agreedToTerms}
            onCheckedChange={setAgreedToTerms}
            required
            disabled={isLoading}
            className="border-border data-[state=checked]:bg-roman-bronze data-[state=checked]:border-roman-bronze"
          />
          <Label
            htmlFor="terms"
            className="text-sm text-muted-foreground cursor-pointer leading-relaxed"
          >
            I agree to the{' '}
            <a
              href="/terms"
              className="text-roman-bronze hover:text-roman-bronze/80 underline"
            >
              Terms of Service
            </a>{' '}
            and{' '}
            <a
              href="/privacy"
              className="text-roman-bronze hover:text-roman-bronze/80 underline"
            >
              Privacy Policy
            </a>
          </Label>
        </div>

        <Button
          type="submit"
          disabled={isLoading || !agreedToTerms}
          className="w-full bg-roman-bronze hover:bg-roman-bronze/90 text-foreground font-bold transition-colors"
        >
          {isLoading ? 'Enlisting...' : 'Join the Legion'}
        </Button>
      </form>

      <div className="mt-6 text-center">
        <p className="text-muted-foreground text-sm">
          Already have an account?{' '}
          <Button
            variant="link"
            onClick={onSwitchToLogin}
            className="text-roman-bronze hover:text-roman-bronze/80 p-0 h-auto font-normal"
          >
            Sign in to Command Center
          </Button>
        </p>
      </div>
    </div>
  );
};

export default SignUpForm;
