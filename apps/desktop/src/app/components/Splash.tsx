import { useEffect, useRef, useState } from "react";

const SPLASH_FLAG = "priority-agent.splash.shown";

export function shouldShowSplash(): boolean {
  try {
    return sessionStorage.getItem(SPLASH_FLAG) !== "1";
  } catch {
    return true;
  }
}

function markSplashShown() {
  try {
    sessionStorage.setItem(SPLASH_FLAG, "1");
  } catch {
    /* sessionStorage unavailable */
  }
}

export function Splash({ onDone }: { onDone: () => void }) {
  const [leaving, setLeaving] = useState(false);
  const onDoneRef = useRef(onDone);
  onDoneRef.current = onDone;

  useEffect(() => {
    const t1 = window.setTimeout(() => setLeaving(true), 1350);
    const t2 = window.setTimeout(() => {
      markSplashShown();
      onDoneRef.current();
    }, 1800);
    return () => {
      window.clearTimeout(t1);
      window.clearTimeout(t2);
    };
  }, []);

  useEffect(() => {
    const skip = (e: KeyboardEvent) => {
      if (e.key !== "Escape" && e.key !== "Enter" && e.key !== " ") return;
      markSplashShown();
      onDoneRef.current();
    };
    window.addEventListener("keydown", skip);
    return () => window.removeEventListener("keydown", skip);
  }, []);

  return (
    <div className={`splash${leaving ? " splash-leaving" : ""}`} onClick={() => { markSplashShown(); onDoneRef.current(); }}>
      <div className="splash-content">
        <div className="splash-icon">◆</div>
        <div className="splash-title">Priority Agent</div>
        <div className="splash-sub">Desktop</div>
      </div>
    </div>
  );
}
