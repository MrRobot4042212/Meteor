import type { GameSource } from './types';
import {
  SteamIcon,
  EpicIcon,
  GogIcon,
  XboxIcon,
  EaIcon,
  UbisoftIcon,
  BattlenetIcon,
  RiotIcon,
  RockstarIcon,
  AmazonIcon,
  WindowsIcon,
  AppIcon,
} from '@/components/icons';

/** Display metadata for each source: label, sidebar icon and the status-dot
 *  colour shown on each card. Used by the Sidebar and GameCard. */
export const SOURCE_META: Record<
  GameSource,
  { label: string; dot: string; Icon: typeof SteamIcon }
> = {
  // Minimalist monochrome: the source is conveyed by the labelled icon in the
  // sidebar; the per-card dot is a neutral marker (its tooltip names the source).
  steam: { label: 'Steam', dot: 'bg-foreground/50', Icon: SteamIcon },
  epic: { label: 'Epic Games', dot: 'bg-foreground/50', Icon: EpicIcon },
  gog: { label: 'GOG', dot: 'bg-foreground/50', Icon: GogIcon },
  ea: { label: 'EA', dot: 'bg-foreground/50', Icon: EaIcon },
  ubisoft: { label: 'Ubisoft', dot: 'bg-foreground/50', Icon: UbisoftIcon },
  xbox: { label: 'Xbox', dot: 'bg-foreground/50', Icon: XboxIcon },
  battlenet: { label: 'Battle.net', dot: 'bg-foreground/50', Icon: BattlenetIcon },
  riot: { label: 'Riot', dot: 'bg-foreground/50', Icon: RiotIcon },
  rockstar: { label: 'Rockstar', dot: 'bg-foreground/50', Icon: RockstarIcon },
  amazon: { label: 'Amazon', dot: 'bg-foreground/50', Icon: AmazonIcon },
  windows: { label: 'Windows', dot: 'bg-foreground/50', Icon: WindowsIcon },
  manual: { label: 'Mis apps', dot: 'bg-foreground/50', Icon: AppIcon },
};

/** Source order used in the sidebar and for dedup priority on the backend. */
export const SOURCE_ORDER: GameSource[] = [
  'steam',
  'epic',
  'gog',
  'xbox',
  'ea',
  'ubisoft',
  'battlenet',
  'riot',
  'rockstar',
  'amazon',
  'windows',
  'manual',
];
