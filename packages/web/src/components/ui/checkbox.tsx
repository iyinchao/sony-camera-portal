import * as CheckboxPrimitive from '@radix-ui/react-checkbox'
import { Check } from 'lucide-react'
import type { ComponentPropsWithoutRef } from 'react'
import { cn } from '@/lib/utils'

// shadcn-style checkbox: a rounded square that fills with the accent and shows a
// check when selected. The translucent frosted backdrop keeps it visible over
// photo thumbnails.
export function Checkbox({
  className,
  ...props
}: ComponentPropsWithoutRef<typeof CheckboxPrimitive.Root>) {
  return (
    <CheckboxPrimitive.Root
      className={cn(
        'peer h-5 w-5 shrink-0 rounded-[6px] border border-black/15 bg-white/85 shadow-sm backdrop-blur-sm transition-colors',
        'outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-1',
        'data-[state=checked]:border-accent data-[state=checked]:bg-accent data-[state=checked]:text-white',
        className,
      )}
      {...props}
    >
      <CheckboxPrimitive.Indicator className="flex items-center justify-center text-current">
        <Check className="h-3.5 w-3.5" strokeWidth={3} />
      </CheckboxPrimitive.Indicator>
    </CheckboxPrimitive.Root>
  )
}
