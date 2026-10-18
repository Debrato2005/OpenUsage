import type { ComponentProps } from 'react';
import { cn } from '../../lib/utils';

export function Spinner({ className, ...props }: ComponentProps<'span'>) {
  return <span role="status" aria-label="Loading" className={cn('ui-spinner', className)} {...props} />;
}
