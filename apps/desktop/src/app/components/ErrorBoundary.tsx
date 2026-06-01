import { Component, type ReactNode } from "react";

type ErrorBoundaryProps = { label: string; children: ReactNode };
type ErrorBoundaryState = { error: Error | null };

export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { error };
  }

  componentDidCatch(error: Error, info: { componentStack?: string | null }): void {
    console.error(`[${this.props.label}] render error`, error, info);
  }

  render(): ReactNode {
    if (!this.state.error) return this.props.children;
    return (
      <div className="error-boundary-fallback">
        <div className="error-boundary-title">Something went wrong</div>
        <div className="error-boundary-panel">{this.props.label}</div>
        <div className="error-boundary-message">{this.state.error.message}</div>
        <button
          type="button"
          className="error-boundary-retry"
          onClick={() => this.setState({ error: null })}
        >
          Retry
        </button>
      </div>
    );
  }
}
