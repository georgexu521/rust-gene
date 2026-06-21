type ExportNoticeBannerProps = {
  notice: string;
  path: string | null;
  onDismiss: () => void;
  onOpen: (path: string) => void;
};

export function ExportNoticeBanner({
  notice,
  path,
  onDismiss,
  onOpen,
}: ExportNoticeBannerProps) {
  return (
    <div className="export-banner" role="status" aria-label="Export complete">
      <span>{notice}</span>
      <div className="export-banner-actions">
        {path ? (
          <button type="button" onClick={() => onOpen(path)}>
            Open export
          </button>
        ) : null}
        <button type="button" onClick={onDismiss}>
          Dismiss
        </button>
      </div>
    </div>
  );
}
