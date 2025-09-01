import React, { useState, useEffect, useRef } from 'react';
import { useUser } from '../../lib/auth-client';
import AuthModal from './AuthModal';

interface RomanShieldLandingProps {
  onSuccess?: () => void;
  redirectTo?: string;
}

const RomanShieldLanding: React.FC<RomanShieldLandingProps> = ({
  onSuccess,
  redirectTo = '/command-center',
}) => {
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [mounted, setMounted] = useState(false);
  const [carouselIndex, setCarouselIndex] = useState(0);
  const [fade, setFade] = useState(true);
  const [isAuthModalOpen, setIsAuthModalOpen] = useState(false);
  const landingRef = useRef<HTMLDivElement>(null);

  const { user: _user, isAuthenticated, isLoading: authLoading } = useUser();

  const carouselWords = [
    'testudo',
    'disciplina',
    'formatio',
    'prudentia',
    'imperium',
  ];

  useEffect(() => {
    setMounted(true);
    const fadeInterval = setInterval(() => {
      setFade(false);
      setTimeout(() => {
        setCarouselIndex(prevIndex => (prevIndex + 1) % carouselWords.length);
        setFade(true);
      }, 1000); // Time for fade out
    }, 4000); // Time word is visible

    return () => clearInterval(fadeInterval);
  }, []);

  // Redirect authenticated users
  useEffect(() => {
    if (isAuthenticated && !authLoading) {
      onSuccess?.();
    }
  }, [isAuthenticated, authLoading, onSuccess]);

  const handleMouseMove = (e: React.MouseEvent<HTMLDivElement>) => {
    if (landingRef.current) {
      landingRef.current.style.setProperty('--mouse-x', `${e.clientX}px`);
      landingRef.current.style.setProperty('--mouse-y', `${e.clientY}px`);
    }
  };

  const handleShieldClick = async () => {
    if (isAuthenticated) {
      // User is already authenticated, redirect them
      onSuccess?.();
      return;
    }

    setIsLoading(true);
    setError(null);

    try {
      setIsAuthModalOpen(true);
    } catch (err) {
      setError('Failed to initialize authentication');
      console.error('Authentication error:', err);
    } finally {
      setIsLoading(false);
    }
  };

  const handleModalClose = () => {
    setIsAuthModalOpen(false);
  };

  const handleAuthSuccess = () => {
    setIsAuthModalOpen(false);
    onSuccess?.();
  };

  // Show loading state while checking authentication
  if (authLoading) {
    return (
      <div
        className="roman-shield-landing"
        style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          minHeight: '100vh',
          backgroundColor: '#000',
        }}
      >
        <div className="loading-spinner"></div>
      </div>
    );
  }

  return (
    <div
      className="roman-shield-landing"
      ref={landingRef}
      onMouseMove={handleMouseMove}
    >
      <div className="background-image"></div>
      <div className="spotlight-overlay"></div>
      <div className="social-links-container">
        <div className="social-links">
          <a
            href="https://twitter.com"
            target="_blank"
            rel="noopener noreferrer"
            className="social-link"
            aria-label="Follow on Twitter"
          >
            <svg width="20" height="20" fill="currentColor" viewBox="0 0 24 24">
              <path d="M18.244 2.25h3.308l-7.227 8.26 8.502 11.24H16.17l-5.214-6.817L4.99 21.75H1.68l7.73-8.835L1.254 2.25H8.08l4.713 6.231zm-1.161 17.52h1.833L7.084 4.126H5.117z" />
            </svg>
          </a>
          <a
            href="https://github.com"
            target="_blank"
            rel="noopener noreferrer"
            className="social-link"
            aria-label="View on GitHub"
          >
            <svg width="20" height="20" fill="currentColor" viewBox="0 0 24 24">
              <path d="M12 0C5.374 0 0 5.373 0 12 0 17.302 3.438 21.8 8.207 23.387c.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23A11.509 11.509 0 0112 5.803c1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.30.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576C20.566 21.797 24 17.3 24 12c0-6.627-5.373-12-12-12z" />
            </svg>
          </a>
        </div>
      </div>
      <div className="carousel-container">
        <div
          className={`carousel-text ${carouselWords[carouselIndex] === 'testudo' ? 'testudo-hue' : ''}`}
        >
          {carouselWords[carouselIndex]}
        </div>
      </div>
      <style>{`
        .roman-shield-landing {
          --mouse-x: 50%;
          --mouse-y: 50%;
          font-family: var(--font-primary, 'Inter', sans-serif);
          color: #F2F2F7;
          min-height: 100vh;
          display: flex;
          align-items: center;
          justify-content: center;
          overflow: hidden;
          position: relative;
        }

        .background-image {
          position: absolute;
          top: 0;
          left: 0;
          width: 100%;
          height: 100%;
          background-image: url('../../assets/Roman-testudo-Trajan-column-966204074.jpg');
          background-size: cover;
          background-position: center;
          z-index: -2;
        }

        .spotlight-overlay {
            position: absolute;
            top: 0;
            left: 0;
            width: 100%;
            height: 100%;
            background: radial-gradient(circle 300px at var(--mouse-x) var(--mouse-y), transparent 0%, rgba(0,0,0,0.9) 100%);
            z-index: -1;
        }

        .hero-container {
          text-align: center;
          max-width: 800px;
          padding: 2rem;
          opacity: ${mounted ? '1' : '0'};
          transform: translateY(${mounted ? '0' : '20px'});
          transition: opacity 0.8s ease, transform 0.8s ease;
          z-index: 1;
        }

        .carousel-container {
          position: absolute;
          top: 2rem;
          left: 2rem;
        }

        .carousel-text {
          font-family: var(--font-display, 'Cinzel', sans-serif);
          font-weight: 700;
          font-size: 1.5rem;
          line-height: 1.2;
          text-shadow: 0 2px 8px rgba(0, 0, 0, 0.5);
          color: #ffffff;
          opacity: ${fade ? 1 : 0};
          transition: opacity 1s ease-in-out;
          text-transform: lowercase;
        }

        .testudo-hue {
          text-shadow: 0 0 10px rgba(220, 20, 60, 0.7);
        }

        .shield-image {
          display: block;
          width: 300px;
          height: auto;
          margin: 2rem auto;
          cursor: pointer;
          transition: all 0.4s cubic-bezier(0.4, 0, 0.2, 1);
          filter: drop-shadow(0 12px 40px rgba(0, 0, 0, 0.6)) drop-shadow(0 0 20px rgba(255, 215, 0, 0.3));
        }

        .shield-image:focus {
          outline: 3px solid #FFD700;
          outline-offset: 4px;
        }


        .shield-image:hover {
          transform: scale(1.05) translateY(-6px);
          filter: 
            drop-shadow(0 20px 60px rgba(0, 0, 0, 0.8))
            drop-shadow(0 0 30px rgba(255, 215, 0, 0.5))
            drop-shadow(0 0 60px rgba(220, 20, 60, 0.3))
            brightness(1.1) contrast(1.1);
        }

        .shield-image:disabled,
        .shield-image.disabled {
          opacity: 0.7;
          cursor: not-allowed;
          transform: none;
          pointer-events: none;
        }

        .loading-spinner {
          display: inline-block;
          width: 2rem;
          height: 2rem;
          border: 3px solid transparent;
          border-top: 3px solid #FFD700;
          border-radius: 50%;
          animation: spin 1s linear infinite;
        }

        @keyframes spin {
          0% { transform: rotate(0deg); }
          100% { transform: rotate(360deg); }
        }

        .social-links-container {
          position: absolute;
          top: 2rem;
          right: 2rem;
        }

        .social-links {
          display: flex;
          gap: var(--space-4, 1rem);
          justify-content: center;
          align-items: center;
        }

        .social-link {
          display: flex;
          align-items: center;
          justify-content: center;
          width: 44px;
          height: 44px;
          background: rgba(255, 255, 255, 0.1);
          border: 1px solid rgba(255, 255, 255, 0.2);
          border-radius: 50%;
          color: #ffffff;
          transition: all 0.3s ease;
          text-decoration: none;
          backdrop-filter: blur(10px);
        }

        .social-link:hover {
          border-color: #ffffff;
          background: rgba(255, 255, 255, 0.2);
          color: #ffffff;
          transform: translateY(-2px);
          box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
        }

        .error-message {
          background: rgba(220, 20, 60, 0.2);
          border: 1px solid rgba(220, 20, 60, 0.4);
          color: #DC143C;
          padding: var(--space-4, 1rem);
          border-radius: var(--space-2, 0.5rem);
          margin-bottom: var(--space-4, 1rem);
          font-size: var(--text-sm, 0.875rem);
          backdrop-filter: blur(10px);
        }

        @media (max-width: 768px) {
          .carousel-text {
            font-size: 1.25rem;
          }
          .shield-image {
            width: 250px;
          }
          .hero-container {
            padding: 1.5rem;
          }
        }

        @media (max-width: 480px) {
          .carousel-text {
            font-size: 1rem;
          }
          .shield-image {
            width: 200px;
          }
        }


        @media (prefers-reduced-motion: reduce) {
          .shield-image, .hero-container {
            transition: none;
          }
        }
      `}</style>

      <link rel="preconnect" href="https://fonts.googleapis.com" />
      <link rel="preconnect" href="https://fonts.gstatic.com" crossOrigin="" />
      <link
        href="https://fonts.googleapis.com/css2?family=Cinzel:wght@400;700&family=Inter:wght@300;400;500&display=swap"
        rel="stylesheet"
      />

      <div className="hero-container">
        {error && <div className="error-message">{error}</div>}

        <div style={{ position: 'relative', display: 'inline-block' }}>
          {isLoading && (
            <div
              className="loading-spinner"
              style={{
                position: 'absolute',
                top: '50%',
                left: '50%',
                transform: 'translate(-50%, -50%)',
                zIndex: 10,
                pointerEvents: 'none',
              }}
            ></div>
          )}
          <img
            src="../../assets/AH3853L-Classical-Roman-Scutum-3292747605-removebg-preview.png"
            alt="Enter the Command Center"
            className={`shield-image ${isLoading ? 'disabled' : ''}`}
            onClick={handleShieldClick}
            onKeyDown={e => {
              if (e.key === 'Enter' || e.key === ' ') {
                e.preventDefault();
                handleShieldClick();
              }
            }}
            role="button"
            tabIndex={0}
            style={{ pointerEvents: isLoading ? 'none' : 'auto' }}
          />
        </div>
      </div>

      {/* Authentication Modal */}
      <AuthModal
        isOpen={isAuthModalOpen}
        onClose={handleModalClose}
        onSuccess={handleAuthSuccess}
        redirectTo={redirectTo}
      />
    </div>
  );
};

export default RomanShieldLanding;
