import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  Activity, AudioLines, BarChart3, Check, ChevronDown, CircleHelp, Cpu, Gamepad2,
  Globe2, Headphones, Home, Languages, Mic, MicOff, Monitor, Music2, Pause,
  Play, Radio, Settings2, ShieldCheck, SkipBack, SkipForward, Volume2, X,
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
  const [playing, setPlaying] = useState(false);
  const [muted, setMuted] = useState(false);
  const [language, setLanguage] = useState<'Русский' | 'English'>('Русский');
  const [compact, setCompact] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [hotkey, setHotkey] = useState(
    () => localStorage.getItem('nexus-hotkey') || 'Insert',
  );
  const [recordingHotkey, setRecordingHotkey] = useState(false);
  const [metrics, setMetrics] = useState<PcMetrics | null>(null);
  const [gameMetrics, setGameMetrics] = useState<GameMetrics | null>(null);
  const [networkMetrics, setNetworkMetrics] = useState<NetworkMetrics | null>(null);

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
    }, 2000);

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
    }, 500);

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
    }, 1000);

    return () => {
      active = false;
      window.clearInterval(timer);
    };
  }, []);

  useEffect(() => {
    if (!recordingHotkey) return;

    const onKeyDown = (event: KeyboardEvent) => {
      event.preventDefault();
      event.stopPropagation();

      if (event.key === 'Escape') {
        setRecordingHotkey(false);
        return;
      }

      if (['Control', 'Shift', 'Alt', 'Meta'].includes(event.key)) return;

      const modifiers = [
        event.ctrlKey ? 'Ctrl' : '',
        event.altKey ? 'Alt' : '',
        event.shiftKey ? 'Shift' : '',
        event.metaKey ? 'Win' : '',
      ].filter(Boolean);

      const keyName =
        event.key === ' '
          ? 'Space'
          : event.key.length === 1
            ? event.key.toUpperCase()
            : event.key;

      const nextHotkey = [...modifiers, keyName].join(' + ');

      setHotkey(nextHotkey);
      localStorage.setItem('nexus-hotkey', nextHotkey);
      setRecordingHotkey(false);
    };

    window.addEventListener('keydown', onKeyDown, true);
    return () => window.removeEventListener('keydown', onKeyDown, true);
  }, [recordingHotkey]);

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
    <main className={compact ? 'app compact' : 'app'}>
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
              aria-label="Close preview"
              onClick={() => setCompact(v => !v)}
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
                    <strong>{label('Нет активного трека', 'No active track')}</strong>
                    <span>
                      {label(
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
                  <button className="icon-button" aria-label="Previous">
                    <SkipBack size={16} />
                  </button>
                  <button
                    className="play-button"
                    aria-label={playing ? 'Pause' : 'Play'}
                    onClick={() => setPlaying(v => !v)}
                  >
                    {playing ? <Pause size={17} /> : <Play size={17} />}
                  </button>
                  <button className="icon-button" aria-label="Next">
                    <SkipForward size={16} />
                  </button>
                  <button className="icon-button favorite" aria-label="Volume">
                    <Volume2 size={16} />
                  </button>
                </div>

                <div className="notice">
                  <CircleHelp size={14} />
                  <span>
                    {label(
                      'Управление станет доступно после подключения медиасессии',
                      'Controls activate when a media session is detected',
                    )}
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
                  <strong>{label('Нет активного звонка', 'No active call')}</strong>
                  <span>
                    {label(
                      'Когда звонок активен, здесь появятся кнопки управления',
                      'Call controls will appear here when a call is active',
                    )}
                  </span>
                </div>

                <div className="call-controls">
                  <button
                    className={`round-control ${muted ? 'selected' : ''}`}
                    onClick={() => setMuted(v => !v)}
                    aria-label="Toggle microphone"
                  >
                    {muted ? <MicOff size={17} /> : <Mic size={17} />}
                  </button>
                  <button className="round-control" aria-label="Deafen">
                    <Headphones size={17} />
                  </button>
                  <button className="round-control hangup" aria-label="End call">
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
                <p>
                  {label(
                    'Автоопределение активного плеера',
                    'Automatic active-player detection',
                  )}
                </p>
              </div>
            </div>
            <div className="empty-state">
              <Music2 size={30} />
              <strong>
                {label(
                  'Медиаплеер пока не подключён',
                  'No media player connected yet',
                )}
              </strong>
              <span>
                {label(
                  'Сначала подключим системные медиасессии Windows, затем проверим доступные возможности SoundCloud API.',
                  'We will connect Windows media sessions first, then verify SoundCloud API capabilities.',
                )}
              </span>
            </div>
          </section>
        )}

        {page === 'discord' && (
          <section className="panel page-panel">
            <div className="panel-title">
              <div className="service-icon discord-icon">
                <Headphones size={18} />
              </div>
              <div>
                <h3>Discord</h3>
                <p>{label('Управление активным звонком', 'Active call controls')}</p>
              </div>
            </div>
            <div className="empty-state">
              <Headphones size={30} />
              <strong>{label('Ожидание интеграции', 'Integration pending')}</strong>
              <span>
                {label(
                  'Начнём с доступных официальных способов отображать звонок, управлять микрофоном и завершать вызов.',
                  'We will start with officially supported call status, microphone and hang-up controls.',
                )}
              </span>
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
                  {recordingHotkey
                    ? label(
                        'Нажми нужную клавишу или сочетание · Esc — отмена',
                        'Press a key or shortcut · Esc to cancel',
                      )
                    : label(
                        'Выбери любое сочетание клавиш',
                        'Choose any key or shortcut',
                      )}
                </span>
              </div>
              <button
                className={`hotkey-button ${recordingHotkey ? 'listening' : ''}`}
                onClick={() => setRecordingHotkey(true)}
              >
                {recordingHotkey
                  ? label('ОЖИДАНИЕ…', 'LISTENING…')
                  : hotkey}
              </button>
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
              <div className="setting-icon">
                <Gamepad2 size={18} />
              </div>
              <div className="setting-copy">
                <strong>
                  {label(
                    'Автозапуск с лаунчерами',
                    'Launch with game launchers',
                  )}
                </strong>
                <span>Steam · Epic Games · TLauncher · VK Play</span>
              </div>
              <span className="setting-badge">
                {label('Настроим', 'Planned')}
              </span>
            </div>

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
