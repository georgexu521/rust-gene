import { useCallback, useState } from "react";
import {
  DesktopContextSnapshot,
  DesktopWorkbenchSnapshot,
  desktopContextSnapshot,
  desktopWorkbenchSnapshot,
} from "../../runtime/desktopApi";

type UseWorkbenchSnapshotsOptions = {
  onError: (error: unknown) => void;
};

export function useWorkbenchSnapshots({ onError }: UseWorkbenchSnapshotsOptions) {
  const [contextSnapshot, setContextSnapshot] = useState<DesktopContextSnapshot | null>(null);
  const [workbenchSnapshot, setWorkbenchSnapshot] = useState<DesktopWorkbenchSnapshot | null>(null);

  const refreshStartupSnapshots = useCallback(async () => {
    const [contextResult, workbenchResult] = await Promise.allSettled([
      desktopContextSnapshot(),
      desktopWorkbenchSnapshot(),
    ]);

    if (contextResult.status === "fulfilled") {
      setContextSnapshot(contextResult.value);
    }
    if (workbenchResult.status === "fulfilled") {
      setWorkbenchSnapshot(workbenchResult.value);
      if (workbenchResult.value.runtime_context) {
        setContextSnapshot(workbenchResult.value.runtime_context);
      }
    }

    const snapshotError =
      contextResult.status === "rejected"
        ? contextResult.reason
        : workbenchResult.status === "rejected"
          ? workbenchResult.reason
          : null;
    if (snapshotError) {
      onError(snapshotError);
    }
  }, [onError]);

  const refreshContextSnapshot = useCallback(async () => {
    try {
      setContextSnapshot(await desktopContextSnapshot());
    } catch (error) {
      onError(error);
    }
  }, [onError]);

  const refreshWorkbenchSnapshot = useCallback(async () => {
    try {
      const snapshot = await desktopWorkbenchSnapshot();
      setWorkbenchSnapshot(snapshot);
      if (snapshot.runtime_context) {
        setContextSnapshot(snapshot.runtime_context);
      }
    } catch (error) {
      onError(error);
    }
  }, [onError]);

  return {
    contextSnapshot,
    refreshContextSnapshot,
    refreshStartupSnapshots,
    refreshWorkbenchSnapshot,
    workbenchSnapshot,
  };
}
