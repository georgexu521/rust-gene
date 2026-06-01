import { useEffect, useMemo, useRef, useState } from "react";
import { TranscriptItem } from "../types";

type JumpBarProps = {
  items: TranscriptItem[];
  threadEl?: HTMLElement | null;
};

type JumpTarget = { turn: number; text: string; idx: number };

export function JumpBar({ items, threadEl }: JumpBarProps) {
  const [hovered, setHovered] = useState<number | null>(null);
  const [active, setActive] = useState<number | null>(null);
  const barRef = useRef<HTMLDivElement>(null);
  const [showPreview, setShowPreview] = useState(false);
  const previewPosRef = useRef(0);

  const targets: JumpTarget[] = useMemo(() => {
    let turn = 0;
    const result: JumpTarget[] = [];
    for (let i = 0; i < items.length; i++) {
      const item = items[i]!;
      if (item.role === "user") {
        turn++;
        result.push({ turn, text: item.text.slice(0, 80), idx: i });
      }
    }
    return result;
  }, [items]);

  useEffect(() => {
    if (targets.length > 0) setActive(targets[targets.length - 1]!.turn);
  }, [targets]);

  useEffect(() => {
    if (!threadEl) return;
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) {
            const turn = Number((entry.target as HTMLElement).dataset.jumpTurn);
            if (!Number.isNaN(turn)) setActive(turn);
          }
        }
      },
      { root: threadEl, threshold: 0.1 },
    );

    const marks = threadEl.querySelectorAll("[data-jump-turn]");
    marks.forEach((m) => observer.observe(m));
    return () => observer.disconnect();
  }, [threadEl, targets]);

  function handleClick(turn: number, idx: number) {
    if (!threadEl) return;
    const el = threadEl.querySelector(`[data-jump-idx="${idx}"]`);
    if (el) {
      el.scrollIntoView({ block: "start", behavior: "smooth" });
    }
  }

  if (targets.length < 2) return null;

  return (
    <div ref={barRef} className="jump-bar" onMouseLeave={() => setHovered(null)}>
      {targets.map((t) => (
        <button
          key={t.turn}
          type="button"
          className={`jump-bar-dot${t.turn === active ? " active" : ""}${t.turn === hovered ? " hovered" : ""}`}
          title={t.text}
          onClick={() => handleClick(t.turn, t.idx)}
          onMouseEnter={(e) => {
            setHovered(t.turn);
            previewPosRef.current = (e.target as HTMLElement).offsetTop;
            setShowPreview(true);
          }}
        />
      ))}
      {showPreview && hovered !== null ? (
        <div
          className="jump-bar-preview"
          style={{ top: previewPosRef.current }}
        >
          {targets.find((t) => t.turn === hovered)?.text}
        </div>
      ) : null}
    </div>
  );
}
