/** One segment of the single/batch mode toggle. */
export function ModeButton({
  active,
  onClick,
  children,
}: {
  active: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      role="radio"
      aria-checked={active}
      onClick={onClick}
      className={`rounded-[10px] px-3 py-1.5 font-inter text-[13px] font-medium transition-colors ${
        active ? "bg-chestnut text-liquid-abyss shadow-bloom" : "text-driftwood hover:text-sepia"
      }`}
    >
      {children}
    </button>
  );
}
