// Bir ekran çökerse tüm uygulama boş kalmasın: hatayı ve kopyalanabilir ayrıntıyı göster.
import { Component, type ReactNode } from "react";

interface Props {
  title: string;
  hint: string;
  copyLabel: string;
  children: ReactNode;
  resetKey: string;
}

export class ErrorBoundary extends Component<Props, { error: Error | null }> {
  state = { error: null as Error | null };

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  componentDidUpdate(prev: Props) {
    // Başka bir ekrana geçilince yeniden dene.
    if (prev.resetKey !== this.props.resetKey && this.state.error) this.setState({ error: null });
  }

  render() {
    const { error } = this.state;
    if (!error) return this.props.children;
    const details = `${error.name}: ${error.message}\n${error.stack ?? ""}`;
    return (
      <div className="offline" style={{ textAlign: "left" }}>
        <h2>{this.props.title}</h2>
        <p>{this.props.hint}</p>
        <pre className="helper-out">{error.message}</pre>
        <button className="btn" onClick={() => void navigator.clipboard.writeText(details)}>
          {this.props.copyLabel}
        </button>
      </div>
    );
  }
}
