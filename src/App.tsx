import { useState } from "react";

const views = ["Translate", "History", "Settings"] as const;
type View = (typeof views)[number];

export function App() {
  const [activeView, setActiveView] = useState<View>("Translate");

  return (
    <div className="app-shell">
      <header className="title-bar">
        <h1>Quick Translate</h1>
      </header>
      <nav className="primary-nav" aria-label="Main navigation">
        {views.map((view) => (
          <button
            key={view}
            type="button"
            className={`nav-button${activeView === view ? " active" : ""}`}
            aria-current={activeView === view ? "page" : undefined}
            onClick={() => setActiveView(view)}
          >
            {view}
          </button>
        ))}
      </nav>
      <main className="main-panel" aria-label={activeView} />
    </div>
  );
}
