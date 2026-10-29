import { useEffect, useState, type MouseEvent } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import {
  Activity, AudioLines, BarChart3, Check, ChevronDown, CircleHelp, Cpu, Power,
  Globe2, Headphones, Home, Languages, Mic, MicOff, Monitor, Music2, Pause,
  Play, Radio, Settings2, ShieldCheck, SkipBack, SkipForward, Volume2, Minus, Plus, X,
  PhoneOff, MemoryStick, Network, Keyboard,
} from 'lucide-react';

type Page = 'home' | 'music' | 'discord' | 'performance' | 'settings';

type PcMetrics = {
  cpu_usage: number;
  ram_used_mb: number;
  ram_total_mb: number;
  gpu_name: string;
  gpu_usage: number | null;
  gpu_memory_used_mb: number | null;
  gpu_memory_total_mb: number | null;
  gpu_temperature: number | null;
};

type GameMetrics = {
  fps: number | null;
  frame_time_ms: number | null;
  game_name: string | null;
};

type NetworkStatus =
  | 'collecting'
  | 'online'
  | 'degraded'
  | 'icmp_blocked'
  | 'offline'
  | 'error';

type NetworkMetrics = {
  ping_ms: number | null;
  packet_loss: number | null;
  endpoint?: string | null;
  status?: NetworkStatus;
  samples?: number;
};

type MediaSession = {
  title: string | null;
  artist: string | null;
  source: string | null;
  playing: boolean;
};

type DiscordVoiceStatus = {
  connected: boolean;
  channel_name: string | null;
  self_mute: boolean;
  self_deaf: boolean;
  participant_count: number;
};

type DiscordConfig = {
  app_id: string | null;
  redirect_uri: string | null;
  configured: boolean;
};

const navItems: { id: Page; label: string; icon: typeof Home }[] = [
  { id: 'home', label: 'Главная', icon: Home },
  { id: 'music', label: 'Музыка', icon: Music2 },
  { id: 'discord', label: 'Discord', icon: Headphones },
  { id: 'performance', label: 'Производительность', icon: BarChart3 },
  { id: 'settings', label: 'Настройки', icon: Settings2 },
];

function Metric({
  label,
  value,
  sub,
  width,
  tone = 'mint',
}: {
  label: string;
  value: string;
  sub?: string;
  width: number;
  tone?: string;
}) {
  const safeWidth = Math.max(0, Math.min(100, width));

  return (
    <div className="metric">
      <div className="metric-label">{label}</div>
      <div className="metric-value">{value}</div>
      {sub && <div className="metric-sub">{sub}</div>}
      <div className="meter">
        <span className={tone} style={{ width: `${safeWidth}%` }} />
      </div>
    </div>
  );
}

export default function App() {
  const [page, setPage] = useState<Page>('home');
  const [mediaSession, setMediaSession] = useState<MediaSession | null>(null);
  const [mediaError, setMediaError] = useState<string | null>(null);
  const [language, setLanguage] = useState<'Русский' | 'English'>('Русский');
  const [showSettings, setShowSettings] = useState(false);
  const [startOnLogin, setStartOnLogin] = useState(false);
  const [startupError, setStartupError] = useState<string | null>(null);
  const hotkey = 'Insert';
  const [metrics, setMetrics] = useState<PcMetrics | null>(null);
  const [gameMetrics, setGameMetrics] = useState<GameMetrics | null>(null);
  const [networkMetrics, setNetworkMetrics] = useState<NetworkMetrics | null>(null);
  const [discordConfig, setDiscordConfig] = useState<DiscordConfig | null>(null);
  const [discordAppId, setDiscordAppId] = useState('');
  const [discordSecret, setDiscordSecret] = useState('');
  const [discordRedirect, setDiscordRedirect] = useState('http://localhost');
  const [discordStatus, setDiscordStatus] = useState<DiscordVoiceStatus | null>(null);
  const [discordAuthorized, setDiscordAuthorized] = useState(false);
  const [discordMessage, setDiscordMessage] = useState<string | null>(null);
  const [discordBusy, setDiscordBusy] = useState(false);

  useEffect(() => {
    let active = true;

    const updateMetrics = async () => {
      try {
        const data = await invoke<PcMetrics>('get_pc_metrics');
        if (active) setMetrics(data);
      } catch (error) {
        console.error('Failed to read PC metrics:', error);
      }
    };

    void updateMetrics();

    const timer = window.setInterval(() => {
      void updateMetrics();
    }, 4000);

    return () => {
      active = false;
      window.clearInterval(timer);
    };
  }, []);

  useEffect(() => {
    let active = true;

    const updateGameMetrics = async () => {
      try {
        const result = await invoke<GameMetrics>('get_game_metrics');
        if (active) setGameMetrics(result);
      } catch (error) {
        console.error('Не удалось получить FPS:', error);
      }
    };

    void updateGameMetrics();

    const timer = window.setInterval(() => {
      void updateGameMetrics();
    }, 1000);

    return () => {
      active = false;
      window.clearInterval(timer);
    };
  }, []);

  useEffect(() => {
    let active = true;

    const updateNetworkMetrics = async () => {
      try {
        const result = await invoke<NetworkMetrics>('get_network_metrics');
        if (active) setNetworkMetrics(result);
      } catch (error) {
        console.error('Не удалось получить сетевые метрики:', error);
      }
    };

    void updateNetworkMetrics();

    const timer = window.setInterval(() => {
      void updateNetworkMetrics();
    }, 2000);

    return () => {
      active = false;
      window.clearInterval(timer);
    };
  }, []);


  useEffect(() => {
    void invoke<boolean>('get_start_on_login')
      .then(setStartOnLogin)
      .catch(() => setStartOnLogin(false));
  }, []);

  const updateStartOnLogin = async () => {
    const next = !startOnLogin;
    try {
      await invoke('set_start_on_login', { enabled: next });
      setStartOnLogin(next);
      setStartupError(null);
    } catch (error) {
      setStartupError(String(error));
    }
  };

  useEffect(() => {
    let active = true;
    void invoke<DiscordConfig>('get_discord_config').then(async config => {
      if (!active) return;
      setDiscordConfig(config);
      if (config.app_id) setDiscordAppId(config.app_id);
      if (config.redirect_uri) setDiscordRedirect(config.redirect_uri);
      if (!config.configured) return;
      try {
        const voice = await invoke<DiscordVoiceStatus>('get_discord_voice_status');
        if (active) {
          setDiscordStatus(voice);
          setDiscordAuthorized(true);
          setDiscordMessage(null);
        }
      } catch (error) {
        if (active) {
          setDiscordAuthorized(false);
          setDiscordMessage(String(error));
        }
      }
    }).catch(error => {
      if (active) setDiscordMessage(String(error));
    });
    return () => { active = false; };
  }, []);

  useEffect(() => {
    if (!discordAuthorized) return;
    const timer = window.setInterval(() => {
      void invoke<DiscordVoiceStatus>('get_discord_voice_status')
        .then(setDiscordStatus)
        .catch(error => {
          setDiscordAuthorized(false);
          setDiscordMessage(String(error));
        });
    }, 3000);
    return () => window.clearInterval(timer);
  }, [discordAuthorized]);

  const saveDiscordCredentials = async () => {
    setDiscordBusy(true);
    try {
      await invoke('save_discord_credentials', {
        appId: discordAppId,
        clientSecret: discordSecret,
        redirectUri: discordRedirect,
      });
      setDiscordSecret('');
      setDiscordConfig({ app_id: discordAppId, redirect_uri: discordRedirect, configured: true });
      setDiscordMessage(label('Credentials сохранены в Windows Credential Manager.', 'Credentials saved in Windows Credential Manager.'));
    } catch (error) {
      setDiscordMessage(String(error));
    } finally {
      setDiscordBusy(false);
    }
  };

  const connectDiscord = async () => {
    setDiscordBusy(true);
    setDiscordMessage(label('Подтвердите подключение в окне Discord…', 'Approve the connection in Discord…'));
    try {
      const voice = await invoke<DiscordVoiceStatus>('connect_discord');
      setDiscordStatus(voice);
      setDiscordAuthorized(true);
      setDiscordMessage(label('Discord RPC подключён.', 'Discord RPC connected.'));
    } catch (error) {
      setDiscordAuthorized(false);
      setDiscordMessage(String(error));
    } finally {
      setDiscordBusy(false);
    }
  };

  const controlDiscord = async (action: string) => {
    setDiscordBusy(true);
    try {
      const voice = await invoke<DiscordVoiceStatus>('control_discord', { action });
      setDiscordStatus(voice);
      setDiscordMessage(null);
    } catch (error) {
      setDiscordMessage(String(error));
    } finally {
      setDiscordBusy(false);
    }
  };

  const disconnectDiscord = async () => {
    setDiscordBusy(true);
    try {
      await invoke('disconnect_discord');
      setDiscordStatus(null);
      setDiscordAuthorized(false);
      setDiscordMessage(label('Discord отключён.', 'Discord disconnected.'));
    } catch (error) {
      setDiscordMessage(String(error));
    } finally {
      setDiscordBusy(false);
    }
  };

  useEffect(() => {
    let active = true;
    const refreshMedia = async () => {
      try {
        const result = await invoke<MediaSession>('get_media_session');
        if (active) {
          setMediaSession(result);
          setMediaError(null);
        }
      } catch (error) {
        if (active) setMediaError(String(error));
      }
    };
    void refreshMedia();
    const timer = window.setInterval(() => void refreshMedia(), 2500);
    return () => { active = false; window.clearInterval(timer); };
  }, []);

  const sendMediaCommand = async (action: string) => {
    try {
      await invoke('control_media', { action });
      setMediaError(null);
      window.setTimeout(() => {
        void invoke<MediaSession>('get_media_session').then(setMediaSession).catch(() => undefined);
      }, 350);
    } catch (error) {
      setMediaError(String(error));
    }
  };

  const changeSystemVolume = async (action: string) => {
    try {
      await invoke('adjust_system_volume', { action });
      setMediaError(null);
    } catch (error) {
      setMediaError(String(error));
    }
  };

  const startWindowDrag = (event: MouseEvent<HTMLElement>) => {
    const target = event.target as HTMLElement;
    if (target.closest('button, a, input, select, textarea, [data-no-drag]')) return;
    try {
      void getCurrentWindow().startDragging().catch(() => undefined);
    } catch { /* Native window APIs are absent in the browser-only preview. */ }
  };

  const hideOverlay = () => {
    try {
      void getCurrentWindow().hide().catch(() => undefined);
    } catch { /* Native window APIs are absent in the browser-only preview. */ }
  };

  const english = language === 'English';
  const label = (ru: string, en: string) => english ? en : ru;

  const cpuUsage = metrics
    ? Math.max(0, Math.min(100, metrics.cpu_usage))
    : 0;

  const gpuUsage = metrics?.gpu_usage != null
    ? Math.max(0, Math.min(100, metrics.gpu_usage))
    : 0;

  const ramUsage = metrics && metrics.ram_total_mb > 0
    ? Math.max(
        0,
        Math.min(100, (metrics.ram_used_mb / metrics.ram_total_mb) * 100),
      )
    : 0;

  const vramUsage =
    metrics?.gpu_memory_used_mb != null &&
    metrics.gpu_memory_total_mb != null &&
    metrics.gpu_memory_total_mb > 0
      ? Math.max(
          0,
          Math.min(
            100,
            (metrics.gpu_memory_used_mb / metrics.gpu_memory_total_mb) * 100,
          ),
        )
      : 0;

  const formatGb = (mb: number) => (mb / 1024).toFixed(1);

  const gameDetected = gameMetrics?.game_name != null;
  const fps = gameMetrics?.fps;
  const frameTime = gameMetrics?.frame_time_ms;
  const ping = networkMetrics?.ping_ms ?? null;
  const packetLoss = networkMetrics?.packet_loss ?? null;
  const networkStatus: NetworkStatus = networkMetrics?.status ?? 'collecting';
  const networkEndpoint = networkMetrics?.endpoint ?? null;
  const networkSamples = networkMetrics?.samples ?? 0;

  const pingValue =
    ping != null
      ? ping === 0
        ? '<1 ms'
        : `${ping} ms`
      : networkStatus === 'collecting'
        ? '…'
        : '— ms';

  const pingSub = (() => {
    if (networkStatus === 'collecting') {
      return label(
        'Сбор данных · публичный узел',
        'Collecting · public node',
      );
    }
    if (networkStatus === 'icmp_blocked') {
      return label(
        'ICMP закрыт · интернет активен',
        'ICMP filtered · internet active',
      );
    }
    if (networkStatus === 'offline') {
      return label(
        'Нет ответа от 1.1.1.1 / 8.8.8.8',
        'No reply from 1.1.1.1 / 8.8.8.8',
      );
    }
    if (networkStatus === 'error') {
      return label(
        'Ошибка сетевого датчика',
        'Network sensor error',
      );
    }
    if (networkEndpoint) {
      return label(
        `Публичный узел · ${networkEndpoint}`,
        `Public node · ${networkEndpoint}`,
      );
    }
    return label(
      'Публичный узел · 1.1.1.1 / 8.8.8.8',
      'Public node · 1.1.1.1 / 8.8.8.8',
    );
  })();

  const packetLossValue =
    networkStatus === 'collecting'
      ? '…'
      : packetLoss != null
        ? `${packetLoss.toFixed(1)} %`
        : '— %';

  const packetLossSub = (() => {
    if (networkStatus === 'collecting') {
      return label('Сбор первых проб…', 'Collecting initial probes…');
    }
    if (networkStatus === 'icmp_blocked') {
      return label(
        'ICMP фильтруется · TCP в норме',
        'ICMP filtered · TCP online',
      );
    }
    if (networkStatus === 'error') {
      return label('Проверка недоступна', 'Check unavailable');
    }
    if (networkSamples > 0) {
      return label(
        `Окно: ${networkSamples}/20 ICMP-проб`,
        `Window: ${networkSamples}/20 ICMP probes`,
      );
    }
    return label('Последние 20 ICMP-проб', 'Last 20 ICMP probes');
  })();

  const networkResourceText = (() => {
    if (networkStatus === 'collecting') {
      return label('Сбор данных…', 'Collecting…');
    }
    if (networkStatus === 'icmp_blocked') {
      return label('ICMP закрыт · Online', 'ICMP filtered · Online');
    }
    if (networkStatus === 'offline') {
      return label('Нет ответа', 'Unreachable');
    }
    if (networkStatus === 'error') {
      return label('Ошибка датчика', 'Sensor error');
    }
    const pingPart =
      ping != null
        ? ping === 0
          ? '<1 ms'
          : `${ping} ms`
        : label('Таймаут', 'Timeout');
    const lossPart =
      packetLoss != null ? ` · ${packetLoss.toFixed(0)}% loss` : '';
    return `${pingPart}${lossPart}`;
  })();

  return (
    <main className="app" onMouseDown={startWindowDrag}>
      <aside className="sidebar">
        <div className="brand">
          <div className="brand-mark">N</div>
          <div>
            <div className="brand-name">NEXUS</div>
            <div className="brand-caption">GAMING OVERLAY</div>
          </div>
        </div>

        <div className="nav-label">
          {label('РАБОЧЕЕ ПРОСТРАНСТВО', 'WORKSPACE')}
        </div>

        <nav>
          {navItems.map(item => {
            const Icon = item.icon;

            return (
              <button
                key={item.id}
                className={`nav-item ${page === item.id ? 'active' : ''}`}
                onClick={() => setPage(item.id)}
              >
                <Icon size={17} strokeWidth={1.8} />
                <span>
                  {label(
                    item.label,
                    ({
                      home: 'Overview',
                      music: 'Music',
                      discord: 'Discord',
                      performance: 'Performance',
                      settings: 'Settings',
                    } as Record<string, string>)[item.id],
                  )}
                </span>
                {page === item.id && <span className="nav-indicator" />}
              </button>
            );
          })}
        </nav>

        <div className="sidebar-bottom">
          <div className="connection">
            <span className="dot" />
            <span>{label('Локальный режим', 'Local mode')}</span>
          </div>
          <div className="version">
            NEXUS <span>0.1.0 · PREVIEW</span>
          </div>
        </div>
      </aside>

      <section className="workspace">
        <header className="topbar">
          <div>
            <div className="eyebrow">
              {label('ИГРОВАЯ СРЕДА', 'GAMING SESSION')}
            </div>
            <h1>
              {label(
                navItems.find(n => n.id === page)?.label ?? 'Главная',
                ({
                  home: 'Overview',
                  music: 'Music',
                  discord: 'Discord',
                  performance: 'Performance',
                  settings: 'Settings',
                } as Record<string, string>)[page],
              )}
            </h1>
          </div>

          <div className="top-actions">
            <div className="game-pill">
              <span className="dot" />
              <span>
                {gameDetected
                  ? gameMetrics.game_name
                  : label('Ожидание игры', 'Waiting for game')}
              </span>
            </div>
            <button
              className="icon-button"
              aria-label="Settings"
              onClick={() => {
                setPage('settings');
                setShowSettings(true);
              }}
            >
              <Settings2 size={17} />
            </button>
            <button
              className="icon-button"
              aria-label="Hide overlay · press Insert to show"
              onClick={hideOverlay}
            >
              <X size={17} />
            </button>
          </div>
        </header>

        {(page === 'home' || page === 'performance') && (
          <>
            <div className="section-heading">
              <div>
                <h2>{label('Мониторинг системы', 'System monitor')}</h2>
                <p>
                  {metrics
                    ? label(
                        `Данные обновляются автоматически · ${metrics.gpu_name}`,
                        `Auto-refreshing data · ${metrics.gpu_name}`,
                      )
                    : label(
                        'Подключение к системным датчикам…',
                        'Connecting to system sensors…',
                      )}
                </p>
              </div>
              <span className="live-tag">
                <span className="dot" />
                {metrics
                  ? label('СИСТЕМА ПОДКЛЮЧЕНА', 'SYSTEM CONNECTED')
                  : label('ПОДКЛЮЧЕНИЕ', 'CONNECTING')}
              </span>
            </div>

            <div className="metrics-grid">
              <Metric
                label="FPS"
                value={fps != null ? String(fps) : '—'}
                sub={
                  fps != null
                    ? label('Текущая частота кадров', 'Current frame rate')
                    : label('Ожидание данных PresentMon', 'Waiting for PresentMon')
                }
                width={fps != null ? Math.min(100, fps / 5) : 0}
              />

              <Metric
                label="FRAME TIME"
                value={frameTime != null ? `${frameTime.toFixed(2)} ms` : '— ms'}
                sub={label('Время кадра', 'Frame duration')}
                width={frameTime != null ? Math.min(100, frameTime * 5) : 0}
                tone="blue"
              />

              <Metric
                label="PING"
                value={pingValue}
                sub={pingSub}
                width={
                  ping != null
                    ? Math.max(0, Math.min(100, 100 - ping))
                    : 0
                }
                tone="purple"
              />

              <Metric
                label={label('ПОТЕРИ ПАКЕТОВ', 'PACKET LOSS')}
                value={packetLossValue}
                sub={packetLossSub}
                width={packetLoss ?? 0}
                tone="blue"
              />

              <Metric
                label="CPU"
                value={metrics ? `${Math.round(cpuUsage)} %` : '— %'}
                sub={label('Загрузка процессора', 'CPU usage')}
                width={cpuUsage}
                tone="purple"
              />

              <Metric
                label="GPU"
                value={
                  metrics?.gpu_usage != null
                    ? `${metrics.gpu_usage} %`
                    : '— %'
                }
                sub={
                  metrics?.gpu_temperature != null
                    ? `${label('Температура', 'Temperature')}: ${metrics.gpu_temperature} °C`
                    : label('Загрузка видеокарты', 'GPU usage')
                }
                width={gpuUsage}
                tone="blue"
              />
            </div>

            <div className="lower-grid">
              <section className="panel music-panel">
                <div className="panel-head">
                  <div className="panel-title">
                    <div className="service-icon soundcloud">
                      <AudioLines size={18} />
                    </div>
                    <div>
                      <h3>SoundCloud</h3>
                      <p>
                        {label('Медиасессия Windows', 'Windows media session')}
                      </p>
                    </div>
                  </div>
                  <span className="subtle-label">
                    {label('МУЗЫКА', 'MUSIC')}
                  </span>
                </div>

                <div className="track-row">
                  <div className="album-art">
                    <Music2 size={24} />
                  </div>
                  <div className="track-info">
                    <strong>{mediaSession?.title || label('Нет активного трека', 'No active track')}</strong>
                    <span>
                      {mediaSession?.artist || mediaSession?.source || label(
                        'Запусти музыку в поддерживаемом плеере',
                        'Start music in a supported player',
                      )}
                    </span>
                    <div className="track-progress">
                      <span />
                    </div>
                  </div>
                </div>

                <div className="player-controls">
                  <button className="icon-button" aria-label="Decrease system volume" title="Volume down" onClick={() => void changeSystemVolume('down')}>
                    <Minus size={15} />
                  </button>
                  <button className="icon-button" aria-label="Previous" title="Previous track" onClick={() => void sendMediaCommand('previous')}>
                    <SkipBack size={16} />
                  </button>
                  <button
                    className="play-button"
                    aria-label={mediaSession?.playing ? 'Pause' : 'Play'}
                    title={mediaSession?.playing ? 'Pause' : 'Play'}
                    onClick={() => void sendMediaCommand('play_pause')}
                  >
                    {mediaSession?.playing ? <Pause size={17} /> : <Play size={17} />}
                  </button>
                  <button className="icon-button" aria-label="Next" title="Next track" onClick={() => void sendMediaCommand('next')}>
                    <SkipForward size={16} />
                  </button>
                  <button className="icon-button" aria-label="Mute or unmute system volume" title="Mute or unmute system volume" onClick={() => void changeSystemVolume('mute')}>
                    <Volume2 size={16} />
                  </button>
                  <button className="icon-button favorite" aria-label="Increase system volume" title="Volume up" onClick={() => void changeSystemVolume('up')}>
                    <Plus size={15} />
                  </button>
                </div>

                <div className="notice">
                  <CircleHelp size={14} />
                  <span>
                    {mediaError || (mediaSession?.source
                      ? `${label('Медиасессия Windows', 'Windows media session')} · ${mediaSession.source}`
                      : label(
                          'Ожидание медиасессии Windows · SoundCloud работает через системные элементы управления',
                          'Waiting for a Windows media session · SoundCloud uses the system media controls',
                        ))}
                  </span>
                </div>
              </section>

              <section className="panel discord-panel">
                <div className="panel-head">
                  <div className="panel-title">
                    <div className="service-icon discord-icon">
                      <Headphones size={18} />
                    </div>
                    <div>
                      <h3>Discord</h3>
                      <p>{label('Состояние звонка', 'Call status')}</p>
                    </div>
                  </div>
                  <span className="subtle-label">VOICE</span>
                </div>

                <div className="call-empty">
                  <div className="call-wave">
                    <Radio size={22} />
                  </div>
                  <strong>{discordStatus?.connected ? discordStatus.channel_name || label('Активный звонок', 'Active call') : label('Нет активного звонка', 'No active call')}</strong>
                  <span>
                    {discordAuthorized
                      ? discordStatus?.connected
                        ? label(`${discordStatus.participant_count} участников`, `${discordStatus.participant_count} participants`)
                        : label('RPC подключён · голосовой канал не выбран', 'RPC connected · no voice channel selected')
                      : discordMessage || label('Подключите официальный Discord RPC в разделе Discord', 'Connect official Discord RPC in the Discord section')}
                  </span>
                </div>

                <div className="call-controls">
                  <button className={`round-control ${discordStatus?.self_mute ? 'selected' : ''}`} aria-label={label('Заглушить микрофон', 'Mute microphone')} title={label('Переключить микрофон Discord', 'Toggle Discord microphone')} disabled={!discordAuthorized || discordBusy} onClick={() => void controlDiscord('mute')}>
                    {discordStatus?.self_mute ? <MicOff size={17} /> : <Mic size={17} />}
                  </button>
                  <button className={`round-control ${discordStatus?.self_deaf ? 'selected' : ''}`} aria-label={label('Заглушить наушники', 'Deafen')} title={label('Переключить звук Discord', 'Toggle Discord audio')} disabled={!discordAuthorized || discordBusy} onClick={() => void controlDiscord('deafen')}>
                    <Headphones size={17} />
                  </button>
                  <button className="round-control hangup" aria-label={label('Завершить звонок', 'Leave voice channel')} title={label('Покинуть голосовой канал Discord', 'Leave Discord voice channel')} disabled={!discordAuthorized || !discordStatus?.connected || discordBusy} onClick={() => void controlDiscord('leave')}>
                    <PhoneOff size={17} />
                  </button>
                </div>

                <div className="notice">
                  <ShieldCheck size={14} />
                  <span>
                    {label(
                      'Только поддерживаемые официальные способы управления',
                      'Officially supported control methods only',
                    )}
                  </span>
                </div>
              </section>
            </div>

            <section className="panel resources">
              <div className="panel-head">
                <div className="panel-title">
                  <div className="service-icon system-icon">
                    <Activity size={18} />
                  </div>
                  <div>
                    <h3>{label('Ресурсы системы', 'System resources')}</h3>
                    <p>
                      {metrics
                        ? label(
                            'Реальные данные с компьютера',
                            'Live data from this PC',
                          )
                        : label(
                            'Ожидание данных системных датчиков',
                            'Waiting for system sensor data',
                          )}
                    </p>
                  </div>
                </div>
                <span className="subtle-label">LOCAL</span>
              </div>

              <div className="resource-grid">
                <div className="resource">
                  <Cpu size={16} />
                  <div>
                    <span>CPU</span>
                    <strong>
                      {metrics ? `${Math.round(cpuUsage)}%` : '—'}
                    </strong>
                  </div>
                  <div className="mini-meter">
                    <span style={{ width: `${cpuUsage}%` }} />
                  </div>
                </div>

                <div className="resource">
                  <Monitor size={16} />
                  <div>
                    <span>GPU / VRAM</span>
                    <strong>
                      {metrics?.gpu_memory_used_mb != null &&
                      metrics.gpu_memory_total_mb != null
                        ? `${formatGb(metrics.gpu_memory_used_mb)} / ${formatGb(metrics.gpu_memory_total_mb)} GB`
                        : '—'}
                    </strong>
                  </div>
                  <div className="mini-meter blue">
                    <span style={{ width: `${vramUsage}%` }} />
                  </div>
                </div>

                <div className="resource">
                  <MemoryStick size={16} />
                  <div>
                    <span>RAM</span>
                    <strong>
                      {metrics
                        ? `${formatGb(metrics.ram_used_mb)} / ${formatGb(metrics.ram_total_mb)} GB`
                        : '—'}
                    </strong>
                  </div>
                  <div className="mini-meter purple">
                    <span style={{ width: `${ramUsage}%` }} />
                  </div>
                </div>

                <div className="resource">
                  <Network size={16} />
                  <div>
                    <span>{label('Сеть', 'Network')}</span>
                    <strong>{networkResourceText}</strong>
                  </div>
                  <div className="mini-meter">
                    <span
                      style={{
                        width: `${Math.max(0, Math.min(100, packetLoss ?? 0))}%`,
                      }}
                    />
                  </div>
                </div>
              </div>
            </section>
          </>
        )}

        {page === 'music' && (
          <section className="panel page-panel">
              <div className="panel-title">
                <div className="service-icon soundcloud">
                  <AudioLines size={18} />
                </div>
                <div>
                  <h3>SoundCloud</h3>
                  <p>{label('Медиасессии Windows · без доступа к аккаунту', 'Windows media sessions · no account access')}</p>
                </div>
              </div>
            <div className="empty-state">
              <Music2 size={30} />
              <strong>{mediaSession?.title || label('Автоопределение медиаплеера', 'Automatic media-player detection')}</strong>
              <span>
                {mediaSession?.source || mediaSession?.title
                  ? `${mediaSession.artist || ''}${mediaSession.source ? ` · ${mediaSession.source}` : ''}`
                  : label(
                      'NEXUS читает только медиасессию Windows (SMTC). Если браузер или приложение SoundCloud публикует её, трек и транспортные кнопки появятся автоматически. Пароль и cookie не запрашиваются.',
                      'NEXUS reads only the Windows media session (SMTC). When a SoundCloud browser/app publishes one, track metadata and transport controls appear automatically. No password or cookies are requested.',
                    )}
              </span>
              <a className="official-link" href="https://soundcloud.com" target="_blank" rel="noreferrer">
                {label('Открыть SoundCloud', 'Open SoundCloud')} ↗
              </a>
              <a className="text-link" href="https://developers.soundcloud.com/docs/api/guide" target="_blank" rel="noreferrer">
                {label('Официальная документация API', 'Official API documentation')} ↗
              </a>
            </div>
          </section>
        )}

        {page === 'discord' && (
          <section className="panel page-panel discord-setup">
            <div className="panel-title">
              <div className="service-icon discord-icon"><Headphones size={18} /></div>
              <div>
                <h3>Discord RPC</h3>
                <p>{label('Официальный локальный RPC через IPC', 'Official local RPC over IPC')}</p>
              </div>
              <span className={`setting-badge ${discordAuthorized ? 'discord-connected' : ''}`}>
                {discordAuthorized ? label('ПОДКЛЮЧЕНО', 'CONNECTED') : label('НЕ ПОДКЛЮЧЕНО', 'NOT CONNECTED')}
              </span>
            </div>
            <p className="discord-help">
              {label(
                'Создайте приложение в Discord Developer Portal. Добавьте redirect URI ниже в OAuth2 и запросите доступ RPC для приложения/тестера. Клиентский секрет сохраняется только в Windows Credential Manager; NEXUS его не отправляет никуда, кроме официального Discord OAuth token endpoint.',
                'Create an app in Discord Developer Portal. Register the redirect URI below under OAuth2 and ensure RPC access for the app/tester. The client secret is stored only in Windows Credential Manager and is sent only to Discord’s official OAuth token endpoint.',
              )}
            </p>
            <div className="discord-form">
              <label>{label('Application ID', 'Application ID')}
                <input value={discordAppId} onChange={event => setDiscordAppId(event.target.value)} placeholder="123456789012345678" inputMode="numeric" autoComplete="off" />
              </label>
              <label>{label('Client Secret', 'Client Secret')}
                <input type="password" value={discordSecret} onChange={event => setDiscordSecret(event.target.value)} placeholder={discordConfig?.configured ? label('Сохранён · введите новый для замены', 'Saved · enter a new one to replace') : '••••••••••••••••'} autoComplete="new-password" />
              </label>
              <label>{label('Redirect URI (точно как в Developer Portal)', 'Redirect URI (must match Developer Portal)')}
                <input value={discordRedirect} onChange={event => setDiscordRedirect(event.target.value)} placeholder="http://localhost" autoComplete="url" />
              </label>
            </div>
            <div className="discord-actions">
              <button className="select-button" onClick={() => void saveDiscordCredentials()} disabled={discordBusy || !discordAppId || !discordSecret || !discordRedirect}>
                {label('Сохранить локально', 'Save locally')}
              </button>
              <button className="official-link discord-connect" onClick={() => void connectDiscord()} disabled={discordBusy || !discordConfig?.configured}>
                {discordBusy ? label('ОЖИДАНИЕ…', 'WAITING…') : label('Подключить Discord', 'Connect Discord')}
              </button>
              {discordAuthorized && <button className="select-button" onClick={() => void disconnectDiscord()} disabled={discordBusy}>{label('Отключить', 'Disconnect')}</button>}
            </div>
            {discordMessage && <div className="notice settings-notice"><CircleHelp size={14} /><span>{discordMessage}</span></div>}
            {discordStatus?.connected && <div className="discord-live-state"><span className="dot" /><strong>{discordStatus.channel_name || label('Активный голосовой канал', 'Active voice channel')}</strong><span>{label(`${discordStatus.participant_count} участников`, `${discordStatus.participant_count} participants`)}</span></div>}
            <div className="discord-doc-links">
              <a className="text-link" href="https://discord.com/developers/applications" target="_blank" rel="noreferrer">{label('Создать приложение Discord', 'Create a Discord application')} ↗</a>
              <a className="text-link" href="https://discord.com/developers/docs/topics/rpc" target="_blank" rel="noreferrer">{label('Документация RPC', 'RPC documentation')} ↗</a>
            </div>
          </section>
        )}

        {page === 'settings' && (
          <section className="panel settings-panel">
            <div className="settings-title">
              <h2>{label('Настройки приложения', 'Application settings')}</h2>
              <p>
                {label(
                  'NEXUS должен работать так, как удобно тебе.',
                  'NEXUS should work the way you want.',
                )}
              </p>
            </div>

            <div className="setting-row">
              <div className="setting-icon">
                <Keyboard size={18} />
              </div>
              <div className="setting-copy">
                <strong>
                  {label('Горячая клавиша оверлея', 'Overlay hotkey')}
                </strong>
                <span>
                  {label(
                    'Глобальная клавиша Windows · повторное нажатие скрывает или показывает оверлей',
                    'Windows global hotkey · press again to hide or show the overlay',
                  )}
                </span>
              </div>
              <kbd className="setting-badge">{hotkey}</kbd>
            </div>

            <div className="setting-row">
              <div className="setting-icon">
                <Languages size={18} />
              </div>
              <div className="setting-copy">
                <strong>{label('Язык интерфейса', 'Interface language')}</strong>
                <span>
                  {label(
                    'Переключение без перезапуска',
                    'Switch without restarting',
                  )}
                </span>
              </div>
              <button
                className="select-button"
                onClick={() =>
                  setLanguage(v => v === 'Русский' ? 'English' : 'Русский')
                }
              >
                {language}
                <ChevronDown size={14} />
              </button>
            </div>

            <div className="setting-row">
              <div className="setting-icon"><Power size={18} /></div>
              <div className="setting-copy">
                <strong>{label('Фоновый наблюдатель игр', 'Background game watcher')}</strong>
                <span>{label('Запускать NEXUS скрыто при входе в Windows; Insert показывает оверлей', 'Start NEXUS hidden at Windows sign-in; Insert reveals the overlay')}</span>
              </div>
              <button
                className={`startup-toggle ${startOnLogin ? 'enabled' : ''}`}
                onClick={() => void updateStartOnLogin()}
                role="switch"
                aria-checked={startOnLogin}
                aria-label={label('Фоновый запуск с Windows', 'Start background watcher with Windows')}
              >
                <span />
              </button>
            </div>
            {startupError && <div className="notice settings-notice"><CircleHelp size={14} /><span>{startupError}</span></div>}

            <div className="setting-row">
              <div className="setting-icon">
                <Globe2 size={18} />
              </div>
              <div className="setting-copy">
                <strong>{label('Локальные данные', 'Local data')}</strong>
                <span>
                  {label(
                    'Настройки хранятся на этом устройстве',
                    'Settings are stored on this device',
                  )}
                </span>
              </div>
              <Check size={17} className="check-icon" />
            </div>

            {showSettings && (
              <div className="notice settings-notice">
                <Check size={14} />
                <span>
                  {label(
                    'Настройки открыты через верхнюю панель.',
                    'Settings opened from the top bar.',
                  )}
                </span>
              </div>
            )}
          </section>
        )}

        <footer className="footer">
          <span>
            <span className="footer-dot" />{' '}
            {metrics
              ? label(
                  'Системная телеметрия активна · локальные данные',
                  'System telemetry active · local data',
                )
              : label(
                  'Ожидание подключения системной телеметрии',
                  'Waiting for system telemetry',
                )}
          </span>
          <span>TAURI · REACT · RUST</span>
        </footer>
      </section>
    </main>
  );
}
