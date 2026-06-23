import { cva, type VariantProps } from 'class-variance-authority'
import type { ButtonHTMLAttributes } from 'react'
import { cn } from '@/lib/utils'

// shadcn-style button, with the .dev/prompt.md "Minimalist Modern" treatment:
// gradient primary, hover lift, accent-tinted shadow, tactile active scale.
const buttonVariants = cva(
  'inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-xl text-sm font-medium transition-all duration-200 outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-2 focus-visible:ring-offset-background disabled:pointer-events-none disabled:opacity-45 active:scale-[0.98]',
  {
    variants: {
      variant: {
        primary:
          'gradient-accent text-white shadow-sm hover:shadow-accent hover:-translate-y-0.5 hover:brightness-105',
        outline:
          'border border-border bg-card text-foreground hover:border-accent/40 hover:shadow-sm',
        ghost: 'text-muted-foreground hover:bg-muted hover:text-foreground',
      },
      size: {
        default: 'h-11 px-5',
        sm: 'h-9 px-3.5 text-[13px]',
        lg: 'h-12 px-6',
        icon: 'h-9 w-9 px-0',
      },
    },
    defaultVariants: { variant: 'outline', size: 'default' },
  },
)

export interface ButtonProps
  extends ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {}

export function Button({ className, variant, size, ...props }: ButtonProps) {
  return <button className={cn(buttonVariants({ variant, size }), className)} {...props} />
}

export { buttonVariants }
