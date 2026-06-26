import { useMemo, useState } from "react";

type VirtualListProps = {
  ariaLabel: string;
  itemCount: number;
  rowHeight: number;
  height: number;
  overscan?: number;
  className?: string;
  renderRow: (index: number) => React.ReactNode;
};

export function VirtualList({
  ariaLabel,
  itemCount,
  rowHeight,
  height,
  overscan = 4,
  className,
  renderRow,
}: VirtualListProps) {
  const [scrollTop, setScrollTop] = useState(0);
  const visibleRange = useMemo(() => {
    const safeRowHeight = Math.max(1, rowHeight);
    const safeHeight = Math.max(safeRowHeight, height);
    const first = Math.max(0, Math.floor(scrollTop / safeRowHeight) - overscan);
    const visibleCount = Math.ceil(safeHeight / safeRowHeight) + overscan * 2;
    const last = Math.min(itemCount, first + visibleCount);
    return { first, last, safeRowHeight };
  }, [height, itemCount, overscan, rowHeight, scrollTop]);

  const rows = [];
  for (let index = visibleRange.first; index < visibleRange.last; index += 1) {
    rows.push(
      <div
        className="virtual-list-row"
        key={index}
        style={{
          height: visibleRange.safeRowHeight,
          transform: `translateY(${index * visibleRange.safeRowHeight}px)`,
        }}
      >
        {renderRow(index)}
      </div>,
    );
  }

  return (
    <div
      aria-label={ariaLabel}
      className={["virtual-list", className].filter(Boolean).join(" ")}
      data-total-items={itemCount}
      onScroll={(event) => setScrollTop(event.currentTarget.scrollTop)}
      style={{ height }}
    >
      <div className="virtual-list-spacer" style={{ height: itemCount * visibleRange.safeRowHeight }}>
        {rows}
      </div>
    </div>
  );
}
