import type { Metadata } from 'next';
import { Oxanium, Source_Code_Pro } from 'next/font/google';
import './globals.css';
import { I18nProvider } from '@/i18n/I18nProvider';

// Oxanium is the UI font (body + display); Source Code Pro for monospace bits.
const oxanium = Oxanium({
  subsets: ['latin'],
  variable: '--font-oxanium',
  display: 'swap',
});

const mono = Source_Code_Pro({
  subsets: ['latin'],
  variable: '--font-mono',
  display: 'swap',
});

export const metadata: Metadata = {
  title: 'Meteor',
  description: 'Tu biblioteca unificada de juegos y apps',
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="es" className={`dark ${oxanium.variable} ${mono.variable}`}>
      <body className="font-sans no-select">
        <I18nProvider>{children}</I18nProvider>
      </body>
    </html>
  );
}
