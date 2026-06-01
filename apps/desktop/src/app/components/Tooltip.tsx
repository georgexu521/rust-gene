import { type ReactNode, useEffect, useRef, useState } from "react";

type TooltipProps = {
  text: string;
  children: ReactNode;
  delayMs?: number;
};

export function Tooltip({ text, children, delayMs = 400 }: TooltipProps) {
  const [show, setShow] = useState(false);
  const [pos, setPos] = useState({ x: 0, y: 0 });
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const ref = useRef<HTMLSpanElement>(null);

  function handleEnter(e: React.MouseEvent) {
    setPos({ x: e.clientX, y: e.clientY });
    timeoutRef.current = setTimeout(() => setShow(true), delayMs);
  }

  function handleMove(e: React.MouseEvent) {
    setPos({ x: e.clientX, y: e.clientY });
  }

  function handleLeave() {
    if (timeoutRef.current) {
      clearTimeout(timeoutRef.current);
      timeoutRef.current = null;
    }
    setShow(false);
  }

  useEffect(() => {
    return () => {
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
      }
    };
  }, []);

  return (
    <span
      ref={ref}
      className="tooltip-host"
      onMouseEnter={handleEnter}
      onMouseMove={handleMove}
      onMouseLeave={handleLeave}
    >
      {children}
      {show ? (
        <span className="tooltip-pop" style={{ left: pos.x, top: pos.y + 18 }}>
          {text}
        </span>
      ) : null}
    </span>
  );
}
