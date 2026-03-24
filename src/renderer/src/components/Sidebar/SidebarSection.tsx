import type { ReactNode } from 'react'
import { ChevronDown, ChevronRight } from 'lucide-react'

interface SidebarSectionProps {
  title: string
  meta?: string
  expanded: boolean
  onToggle: () => void
  children: ReactNode
}

export function SidebarSection({
  title,
  meta,
  expanded,
  onToggle,
  children
}: SidebarSectionProps): JSX.Element {
  return (
    <section className="shrink-0 border-b border-black/40 bg-black/10">
      <button
        className="flex w-full items-center justify-between gap-3 px-4 py-3 text-left text-[11px] font-bold uppercase tracking-[0.24em] text-sentinel-mist/70 transition hover:bg-white/[0.04] hover:text-white focus:outline-none focus:bg-white/[0.02]"
        onClick={onToggle}
        type="button"
      >
        <span>{title}</span>
        <span className="flex items-center gap-2">
          {meta && <span className="text-[10px] uppercase tracking-[0.2em] text-white/40">{meta}</span>}
          {expanded ? <ChevronDown className="h-3.5 w-3.5" /> : <ChevronRight className="h-3.5 w-3.5" />}
        </span>
      </button>

      <div className={`overflow-hidden transition-all duration-300 ease-out ${expanded ? 'max-h-[28rem] opacity-100' : 'max-h-0 opacity-0'}`}>
        <div className="px-4 pb-4">{children}</div>
      </div>
    </section>
  )
}
