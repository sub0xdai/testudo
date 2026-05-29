/** @anchor ui:journal:AddExchangeCard
 * @tags ui */

export function AddExchangeCard(props: { onClick: () => void }) {
  return (
    <button
      onClick={props.onClick}
      class="border border-dashed border-text-tertiary/30 bg-container-bg p-8 flex flex-col items-center justify-center gap-3 min-h-[200px] hover:border-text-secondary transition-colors group"
    >
      <span class="text-2xl text-text-tertiary group-hover:text-text-primary transition-colors">+</span>
      <span class="text-[10px] font-mono text-text-tertiary group-hover:text-text-primary tracking-[0.2em] uppercase transition-colors">
        ADD EXCHANGE
      </span>
    </button>
  )
}



