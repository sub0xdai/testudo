export function AddExchangeCard(props: { onClick: () => void }) {
  return (
    <button
      onClick={props.onClick}
      class="border border-dashed border-text-tertiary/50 glass-panel p-8 flex flex-col items-center justify-center gap-3 min-h-[200px] hover:border-text-secondary transition-colors group"
    >
      <span class="text-2xl text-text-secondary group-hover:text-text-primary">+</span>
      <span class="text-xs font-mono text-text-secondary group-hover:text-text-primary tracking-wider">
        ADD EXCHANGE
      </span>
    </button>
  )
}
