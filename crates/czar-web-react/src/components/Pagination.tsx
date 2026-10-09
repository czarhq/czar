interface PaginationProps {
  currentPage: number;
  totalPages: number;
  onPageChange: (page: number) => void;
}

export default function Pagination({ currentPage, totalPages, onPageChange }: PaginationProps) {
  if (totalPages <= 1) return null;

  return (
    <div style={{ padding: '8px 12px', borderTop: '1px solid var(--border)', textAlign: 'center', fontSize: '14px', display: 'flex', alignItems: 'center', justifyContent: 'center', gap: '8px' }}>
      {currentPage > 0 && (
        <button
          onClick={() => onPageChange(currentPage - 1)}
          style={{ padding: '4px 8px', background: 'var(--panel-2)', border: '1px solid var(--border)', borderRadius: '4px', cursor: 'pointer', color: 'var(--text)', fontFamily: 'inherit' }}
        >
          Prev
        </button>
      )}
      <span style={{ padding: '0 8px', color: 'var(--muted)' }}>
        Page {currentPage + 1} of {totalPages}
      </span>
      {currentPage < totalPages - 1 && (
        <button
          onClick={() => onPageChange(currentPage + 1)}
          style={{ padding: '4px 8px', background: 'var(--panel-2)', border: '1px solid var(--border)', borderRadius: '4px', cursor: 'pointer', color: 'var(--text)', fontFamily: 'inherit' }}
        >
          Next
        </button>
      )}
    </div>
  );
}
