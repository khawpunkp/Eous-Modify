import { type ClassValue, clsx } from 'clsx';
import { twMerge } from 'tailwind-merge';

export function cn(...inputs: ClassValue[]) {
   return twMerge(clsx(inputs));
}

export const normalize = (text: string) => {
   return text
      .toLowerCase()
      .replace(/[.\-/()\s]/g, '') // remove punctuation
      .normalize('NFKC'); // normalize unicode
};
