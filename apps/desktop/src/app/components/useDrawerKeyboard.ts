import { RefObject, useEffect, useRef } from "react";

type DrawerKeyboardOptions = {
  isOpen: boolean;
  onClose: () => void;
  initialFocusRef?: RefObject<HTMLElement | null>;
};

const FOCUSABLE_SELECTOR = [
  "button:not([disabled])",
  "input:not([disabled])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  "a[href]",
  '[tabindex]:not([tabindex="-1"])',
].join(",");

export function useDrawerKeyboard<T extends HTMLElement>({
  isOpen,
  onClose,
  initialFocusRef,
}: DrawerKeyboardOptions) {
  const drawerRef = useRef<T>(null);

  useEffect(() => {
    if (!isOpen) {
      return;
    }

    const previousFocus =
      document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const focusTimer = window.setTimeout(() => {
      const drawer = drawerRef.current;
      const fallbackFocus = drawer ? findFocusableElements(drawer)[0] : null;
      (initialFocusRef?.current || fallbackFocus || drawer)?.focus();
    }, 0);

    const onKeyDown = (event: KeyboardEvent) => {
      if (!drawerShouldHandleKeyDown(drawerRef.current)) {
        return;
      }

      if (event.key === "Escape") {
        event.preventDefault();
        onClose();
        return;
      }

      if (event.key === "Tab") {
        trapFocusInsideDrawer(event, drawerRef.current);
      }
    };

    document.addEventListener("keydown", onKeyDown);
    return () => {
      window.clearTimeout(focusTimer);
      document.removeEventListener("keydown", onKeyDown);
      previousFocus?.focus();
    };
  }, [initialFocusRef, isOpen, onClose]);

  return drawerRef;
}

function drawerShouldHandleKeyDown(drawer: HTMLElement | null) {
  if (!drawer) {
    return false;
  }
  const activeElement =
    document.activeElement instanceof HTMLElement ? document.activeElement : null;
  if (activeElement && drawer.contains(activeElement)) {
    return true;
  }

  const activeOverlay = activeElement?.closest('aside[aria-label], [role="dialog"]');
  return !activeOverlay;
}

function trapFocusInsideDrawer(event: KeyboardEvent, drawer: HTMLElement | null) {
  if (!drawer) {
    return;
  }

  const focusable = findFocusableElements(drawer);
  if (!focusable.length) {
    event.preventDefault();
    drawer.focus();
    return;
  }

  const first = focusable[0];
  const last = focusable[focusable.length - 1];
  const activeElement =
    document.activeElement instanceof HTMLElement ? document.activeElement : null;

  if (event.shiftKey && activeElement === first) {
    event.preventDefault();
    last.focus();
  } else if (!event.shiftKey && activeElement === last) {
    event.preventDefault();
    first.focus();
  }
}

function findFocusableElements(drawer: HTMLElement) {
  return Array.from(drawer.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)).filter(
    (element) => element.getClientRects().length > 0,
  );
}
