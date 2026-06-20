import type { Metadata } from 'next';
import { Inter, Space_Grotesk } from 'next/font/google';
import './globals.css';

const body = Inter({
  subsets: ['latin'],
  variable: '--font-body',
  display: 'swap',
});

const display = Space_Grotesk({
  subsets: ['latin'],
  variable: '--font-display',
  display: 'swap',
});

export const metadata: Metadata = {
  title: 'Nexo',
  description: 'Tu biblioteca unificada de juegos y apps',
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="es" className={`${body.variable} ${display.variable}`}>
      <body className="font-sans no-select">{children}</body>
    </html>
  );
}
