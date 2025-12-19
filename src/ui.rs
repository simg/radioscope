use dioxus::core::NoOpMutations;
use dioxus::prelude::*;

#[derive(Props, Clone, PartialEq)]
pub struct AppProps {
    pub monitor_interface: String,
}

pub fn render_html(monitor_interface: &str) -> String {
    let mut app = VirtualDom::new_with_props(
        App,
        AppProps {
            monitor_interface: monitor_interface.to_string(),
        },
    );
    // Build the tree before rendering to avoid SSR panics.
    let mut noop = NoOpMutations {};
    let _ = app.rebuild(&mut noop);
    dioxus_ssr::render(&mut app)
}

#[component]
fn App(props: AppProps) -> Element {
    let styles = r#"
:root {
    color-scheme: light;
}
* { box-sizing: border-box; }
body, html {
    margin: 0;
    padding: 0;
    background: radial-gradient(circle at 20% 20%, #171a24, #0b0d13 40%), #0b0d13;
}
.page { min-height: 100vh; display: flex; justify-content: center; padding: 36px 18px; color: #e9ecf5; font-family: "Space Grotesk", "Inter", system-ui, -apple-system, sans-serif; }
.shell { width: min(900px, 100%); display: flex; flex-direction: column; gap: 12px; }
.header { display: flex; flex-direction: column; gap: 6px; }
.title { font-size: 26px; margin: 0; letter-spacing: 0.4px; }
.subtitle { margin: 0; color: #9aa4bc; font-size: 15px; }
.tag { display: inline-flex; align-items: center; gap: 8px; width: fit-content; padding: 8px 12px; border-radius: 999px; background: #10131c; border: 1px solid #1f2431; color: #c5cee3; font-size: 14px; }
.nav { display: flex; flex-wrap: wrap; gap: 10px; }
.nav-btn { padding: 10px 14px; border-radius: 12px; border: 1px solid #1f2230; background: #11131b; color: #c5cee3; font-weight: 700; letter-spacing: 0.2px; cursor: pointer; transition: transform 120ms ease, background 140ms ease, border 140ms ease; }
.nav-btn.active { background: linear-gradient(135deg, #ff5f7a, #ff3c5a); color: #0a0c12; border-color: #ff90a3; box-shadow: 0 12px 28px rgba(255, 79, 100, 0.28); transform: translateY(-1px); }
.content { display: flex; flex-direction: column; gap: 12px; }
.card { width: 100%; background: linear-gradient(145deg, #161a23, #0f1219); border: 1px solid #1f2230; border-radius: 16px; padding: 22px; box-shadow: 0 18px 44px rgba(0,0,0,0.35); }
.section { display: none; }
.section.active { display: block; }
.card-title { margin: 0 0 4px 0; font-size: 20px; }
.muted { color: #8f98ac; margin: 0 0 16px 0; font-size: 14px; }
.channel-groups { display: grid; grid-template-columns: repeat(auto-fit, minmax(240px, 1fr)); gap: 12px; }
.group { background: #10141d; border: 1px solid #1f2230; border-radius: 12px; padding: 12px; }
.group-title { margin: 0 0 10px 0; color: #c5cee3; font-size: 14px; letter-spacing: 0.3px; }
.channel-buttons { display: flex; flex-wrap: wrap; gap: 8px; }
.channel-btn { padding: 10px 12px; border-radius: 10px; border: 1px solid #262b38; background: #0f1118; color: #dfe4f3; font-weight: 700; cursor: pointer; transition: all 120ms ease; min-width: 52px; font-size: 14px; }
.channel-btn.active { background: #ff4f64; border-color: #ff90a3; color: #0b0d12; box-shadow: 0 10px 24px rgba(255,79,100,0.28); }
.channel-btn:disabled { opacity: 0.6; cursor: not-allowed; }
.channel-btn.disabled { opacity: 0.35; filter: blur(0.3px); border-style: dashed; cursor: not-allowed; }
.sound-options { display: grid; grid-template-columns: repeat(auto-fit, minmax(240px, 1fr)); gap: 12px; }
.checkbox { display: flex; align-items: center; gap: 10px; padding: 12px; background: #10141d; border: 1px solid #1f2230; border-radius: 12px; cursor: pointer; }
.checkbox input { width: 18px; height: 18px; }
.primary { width: 100%; padding: 14px 16px; border-radius: 12px; border: none; background: linear-gradient(135deg, #ff5f7a, #ff3c5a); color: #0b0d12; font-weight: 800; font-size: 16px; letter-spacing: 0.3px; box-shadow: 0 12px 30px rgba(255,79,100,0.35); transition: transform 120ms ease, box-shadow 120ms ease, filter 120ms ease; cursor: pointer; }
.primary:active { transform: translateY(1px); box-shadow: 0 8px 20px rgba(255,79,100,0.28); filter: brightness(0.95); }
.status { margin-top: 10px; color: #8f98ac; font-size: 14px; min-height: 18px; }
.caption { margin: 6px 0 0 0; color: #7c859c; font-size: 13px; }
.mode-row { display: flex; gap: 12px; align-items: center; margin: 8px 0 12px 0; flex-wrap: wrap; }
.mode-select { padding: 10px 12px; border-radius: 10px; border: 1px solid #262b38; background: #0f1118; color: #dfe4f3; font-weight: 700; }
.packet-list { display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 10px; }
.packet-item { display: flex; align-items: center; gap: 10px; padding: 12px; background: #10141d; border: 1px solid #1f2230; border-radius: 12px; }
.packet-item input { width: 18px; height: 18px; }
.packet-item .spacer { flex: 1; }
.pill-btn { padding: 8px 10px; border-radius: 10px; border: 1px solid #262b38; background: #0f1118; color: #dfe4f3; font-weight: 700; cursor: pointer; transition: all 120ms ease; }
.pill-btn:hover { border-color: #ff90a3; color: #ffb5c2; }
.packet-actions { display: flex; gap: 10px; flex-wrap: wrap; margin-bottom: 8px; }
.device-controls { display: flex; gap: 10px; align-items: center; flex-wrap: wrap; margin: 4px 0 8px 0; }
.device-window { display: inline-flex; align-items: center; gap: 8px; padding: 10px 12px; background: #10141d; border: 1px solid #1f2230; border-radius: 10px; color: #c5cee3; font-size: 13px; }
.device-window input { width: 80px; padding: 8px 10px; border-radius: 8px; border: 1px solid #262b38; background: #0f1118; color: #e9ecf5; font-weight: 700; }
.device-actions { display: flex; gap: 10px; flex-wrap: wrap; margin: 6px 0 10px 0; }
.device-groups { display: flex; flex-direction: column; gap: 10px; }
.device-group { background: #10141d; border: 1px solid #1f2230; border-radius: 12px; padding: 10px; }
.device-group-title { margin: 0 0 8px 0; font-size: 13px; color: #9aa4bc; letter-spacing: 0.4px; text-transform: uppercase; }
.device-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(240px, 1fr)); gap: 8px; }
.device-card { display: flex; gap: 10px; align-items: flex-start; padding: 8px 10px; border-radius: 12px; border: 1px solid #1f2230; background: #0f1218; min-height: 48px; }
.device-card input { width: 16px; height: 16px; margin-top: 2px; }
.device-body { display: flex; flex-direction: column; gap: 4px; flex: 1; min-width: 0; }
.device-mac { font-weight: 700; font-size: 13px; color: #f0f2fb; letter-spacing: 0.3px; }
.device-meta { display: flex; flex-wrap: wrap; gap: 8px; font-size: 11px; color: #8f98ac; }
.modal { position: fixed; inset: 0; display: flex; align-items: center; justify-content: center; background: rgba(5,7,12,0.72); backdrop-filter: blur(6px); padding: 18px; }
.modal.hidden { display: none; }
.modal-card { width: min(420px, 100%); background: #0f121a; border: 1px solid #1f2230; border-radius: 14px; padding: 20px; box-shadow: 0 18px 40px rgba(0,0,0,0.4); }
.modal-title { margin: 0 0 10px 0; color: #f8f9fb; font-size: 18px; }
.modal-body { color: #c5cee3; margin: 0 0 18px 0; line-height: 1.4; }
.actions { display: flex; gap: 10px; }
.ghost, .danger { flex: 1; padding: 12px 14px; border-radius: 10px; font-weight: 700; font-size: 15px; cursor: pointer; }
.ghost { background: #161925; border: 1px solid #262c3a; color: #c5cee3; }
.danger { background: #ff4f64; border: none; color: #0b0d12; }
@media (max-width: 640px) {
    .page { padding: 20px 14px; }
    .shell { gap: 10px; }
    .card { padding: 18px; }
    .title { font-size: 22px; }
    .subtitle { font-size: 14px; }
    .nav-btn { flex: 1; text-align: center; padding: 10px 12px; }
    .card-title { font-size: 18px; }
}
@media (max-width: 440px) {
    .page { padding: 16px 12px; }
    .card { padding: 16px; }
    .title { font-size: 20px; }
    .channel-groups { gap: 10px; }
    .channel-btn { padding: 9px 10px; font-size: 13px; min-width: 48px; }
    .checkbox { padding: 10px; }
    .primary { font-size: 15px; }
}
"#;

    let script = r#"
(() => {
  const navButtons = document.querySelectorAll('[data-target]');
  const sections = document.querySelectorAll('[data-section]');
  const channel24 = document.getElementById('channels-24');
  const channel5 = document.getElementById('channels-5');
  let channelButtons = [];
  const channelStatus = document.getElementById('channel-status');
  const soundStatus = document.getElementById('sound-status');
  const audioJack = document.getElementById('audio-jack');
  const webUi = document.getElementById('web-ui');
  const volumeBySignal = document.getElementById('volume-by-signal');
  const packetList = document.getElementById('packet-list');
  const packetStatus = document.getElementById('packet-status');
  const modeSelect = document.getElementById('mode-select');
  const toggleAll = document.getElementById('packets-toggle-all');
  const modal = document.getElementById('shutdown-modal');
  const open = document.getElementById('shutdown-btn');
  const cancel = document.getElementById('cancel-btn');
  const confirm = document.getElementById('confirm-btn');
  const shutdownStatus = document.getElementById('status');
  const deviceList = document.getElementById('device-list');
  const deviceStatus = document.getElementById('device-status');
  const deviceWindow = document.getElementById('device-window');
  const deviceRefresh = document.getElementById('devices-refresh');
  const deviceSelectAll = document.getElementById('devices-select-all');
  const deviceDeselectAll = document.getElementById('devices-deselect-all');
  const deviceReset = document.getElementById('devices-reset');
  let ws;
  let deviceWs;
  let audioCtx;
  let packetsState = [];
  let devicesState = [];

  function setSection(target) {
    sections.forEach((section) => {
      const isTarget = section.dataset.section === target;
      section.classList.toggle('active', isTarget);
    });
    navButtons.forEach((btn) => {
      const isTarget = btn.dataset.target === target;
      btn.classList.toggle('active', isTarget);
    });
    if (target === 'devices') {
      fetchDevices();
      ensureDeviceSocket();
    }
  }

  navButtons.forEach((btn) => {
    btn.addEventListener('click', () => setSection(btn.dataset.target));
  });

  async function fetchSettings() {
    try {
      const res = await fetch('/api/settings');
      if (!res.ok) throw new Error('settings failed');
      const data = await res.json();
      renderChannels(data.available_channels_24ghz || [], channel24);
      renderChannels(data.available_channels_5ghz || [], channel5);
      if (typeof data.channel === 'number') {
        setActiveChannel(data.channel);
      }
      audioJack.checked = !!data.audio_jack;
      webUi.checked = !!data.web_ui_sound;
      volumeBySignal.checked = !!data.volume_by_signal;
      packetsState = data.packet_events || [];
      renderPackets(packetsState);
      if (modeSelect && data.mode) {
        modeSelect.value = data.mode;
      }
      if (data.data_tick_n) {
        packetStatus.textContent = `Data tick every ${data.data_tick_n} frames (${data.mode || ''})`;
      }
      if (webUi.checked) {
        ensureWebsocket();
      }
    } catch (err) {
      channelStatus.textContent = 'Unable to load settings';
    }
  }

  function renderChannels(list, container) {
    if (!container) return;
    container.innerHTML = '';
    channelButtons = channelButtons.filter((btn) => btn.isConnected);
    if (!list.length) {
      const msg = document.createElement('div');
      msg.className = 'caption';
      msg.textContent = 'Not supported on this device';
      container.appendChild(msg);
      return;
    }
    const anyEnabled = list.some((item) => item.enabled);
    list.forEach((item) => {
      const channel = item.channel;
      const enabled = !!item.enabled;
      const btn = document.createElement('button');
      btn.className = 'channel-btn';
      if (!enabled) {
        btn.classList.add('disabled');
        btn.disabled = true;
      }
      btn.dataset.channel = channel;
      btn.textContent = channel;
      if (enabled) {
        btn.addEventListener('click', () => setChannel(btn, channel));
      }
      container.appendChild(btn);
      channelButtons.push(btn);
    });
    if (!anyEnabled) {
      const msg = document.createElement('div');
      msg.className = 'caption';
      msg.textContent = 'Not supported on this device';
      container.appendChild(msg);
    }
  }

  function renderPackets(list) {
    if (!packetList) return;
    packetList.innerHTML = '';
    const allOn = list.every((i) => !!i.enabled);
    if (toggleAll) {
      toggleAll.textContent = allOn ? 'Deselect all' : 'Select all';
    }
    list.forEach((item) => {
      const wrap = document.createElement('label');
      wrap.className = 'packet-item';
      const input = document.createElement('input');
      input.type = 'checkbox';
      input.checked = !!item.enabled;
      input.dataset.id = item.id;
      input.addEventListener('change', () => {
        const id = input.dataset.id;
        packetsState = packetsState.map((p) => p.id === id ? { ...p, enabled: input.checked } : p);
        savePackets();
      });
      const span = document.createElement('span');
      span.textContent = item.label || item.id;
      const play = document.createElement('button');
      play.type = 'button';
      play.className = 'pill-btn';
      play.textContent = 'Play';
      play.addEventListener('click', (e) => {
        e.preventDefault();
        playEventSound(item.id, false);
      });
      wrap.appendChild(input);
      wrap.appendChild(span);
      const spacer = document.createElement('div');
      spacer.className = 'spacer';
      wrap.appendChild(spacer);
      wrap.appendChild(play);
      packetList.appendChild(wrap);
    });
  }

  function renderDevices(list) {
    if (!deviceList) return;
    deviceList.innerHTML = '';
    if (!list.length) {
      const empty = document.createElement('div');
      empty.className = 'caption';
      empty.textContent = 'No devices seen in this window yet';
      deviceList.appendChild(empty);
      return;
    }
    const groups = [
      { key: 'ap', label: 'Access points' },
      { key: 'client', label: 'Clients' },
      { key: 'unknown', label: 'Other' },
    ];
    groups.forEach((group) => {
      const items = list.filter((d) => (d.role || '').toLowerCase() === group.key);
      if (!items.length) return;
      const wrap = document.createElement('div');
      wrap.className = 'device-group';
      const title = document.createElement('p');
      title.className = 'device-group-title';
      title.textContent = `${group.label} (${items.length})`;
      wrap.appendChild(title);
      const grid = document.createElement('div');
      grid.className = 'device-grid';
      items.forEach((item) => {
        const card = document.createElement('label');
        card.className = 'device-card';
        const input = document.createElement('input');
        input.type = 'checkbox';
        input.checked = !!item.allowed;
        input.addEventListener('change', () => {
          setDeviceFilter([{ mac: item.mac, allowed: input.checked }], true);
        });
        const body = document.createElement('div');
        body.className = 'device-body';
        const topRow = document.createElement('div');
        topRow.style.display = 'flex';
        topRow.style.alignItems = 'center';
        topRow.style.gap = '8px';
        const mac = document.createElement('div');
        mac.className = 'device-mac';
        mac.textContent = item.mac;
        topRow.appendChild(mac);
        const meta = document.createElement('div');
        meta.className = 'device-meta';
        if (item.ssid) {
          const ssid = document.createElement('span');
          ssid.textContent = `SSID ${item.ssid}`;
          meta.appendChild(ssid);
        }
        if (item.channel) {
          const ch = document.createElement('span');
          ch.textContent = `Ch ${item.channel}`;
          meta.appendChild(ch);
        }
        if (item.bssid) {
          const bssid = document.createElement('span');
          bssid.textContent = `BSSID ${item.bssid}`;
          meta.appendChild(bssid);
        }
        const rssi = document.createElement('span');
        rssi.textContent = item.rssi_dbm != null ? `${item.rssi_dbm} dBm` : 'RSSI n/a';
        meta.appendChild(rssi);
        const age = document.createElement('span');
        const seconds = Math.round((item.age_ms || 0) / 1000);
        age.textContent = seconds <= 1 ? 'just now' : `${seconds}s ago`;
        meta.appendChild(age);
        const frames = document.createElement('span');
        frames.textContent = `${item.frames || 0} frames`;
        meta.appendChild(frames);
        body.appendChild(topRow);
        body.appendChild(meta);
        card.appendChild(input);
        card.appendChild(body);
        grid.appendChild(card);
      });
      wrap.appendChild(grid);
      deviceList.appendChild(wrap);
    });
  }

  function setActiveChannel(channel) {
    channelButtons.forEach((btn) => {
      const isActive = Number(btn.dataset.channel) === Number(channel);
      btn.classList.toggle('active', isActive);
    });
  }

  async function setChannel(btn, channel) {
    if (btn && btn.disabled) return;
    channelStatus.textContent = `Setting channel ${channel}...`;
    channelButtons.forEach((b) => (b.disabled = true));
    try {
      const res = await fetch('/api/channel', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ channel }),
      });
      if (!res.ok) throw new Error('channel update failed');
      const data = await res.json();
      setActiveChannel(data.channel);
      channelStatus.textContent = `Switched to channel ${data.channel}`;
    } catch (err) {
      channelStatus.textContent = 'Failed to change channel';
    } finally {
      channelButtons.forEach((b) => (b.disabled = false));
    }
  }

  function deviceWindowSeconds() {
    const val = parseInt(deviceWindow?.value || '10', 10) || 10;
    const clamped = Math.max(1, Math.min(120, val));
    if (deviceWindow) deviceWindow.value = clamped;
    return clamped * 60;
  }

  async function fetchDevices() {
    if (!deviceList) return;
    const windowSeconds = deviceWindowSeconds();
    deviceStatus.textContent = 'Loading devices...';
    try {
      const res = await fetch(`/api/devices?window_minutes=${windowSeconds / 60}`);
      if (!res.ok) throw new Error('device fetch failed');
      const data = await res.json();
      devicesState = data.devices || [];
      renderDevices(devicesState);
      deviceStatus.textContent = devicesState.length ? '' : 'No devices in this window yet';
    } catch (err) {
      deviceStatus.textContent = 'Unable to load devices';
    }
  }

  function openDeviceSocket() {
    const windowSeconds = deviceWindowSeconds();
    const proto = location.protocol === 'https:' ? 'wss' : 'ws';
    deviceStatus.textContent = 'Connecting...';
    deviceWs = new WebSocket(`${proto}://${location.host}/ws/devices?window_minutes=${windowSeconds / 60}`);
    deviceWs.onmessage = (evt) => {
      try {
        const data = JSON.parse(evt.data);
        devicesState = data.devices || [];
        renderDevices(devicesState);
        deviceStatus.textContent = devicesState.length ? '' : 'No devices in this window yet';
        if (deviceWindow && data.window_seconds) {
          deviceWindow.value = Math.round((data.window_seconds || 600) / 60);
        }
      } catch {
        deviceStatus.textContent = 'Failed to parse devices';
      }
    };
    deviceWs.onerror = () => deviceWs && deviceWs.close();
    deviceWs.onclose = () => {
      if (!deviceWs) return;
      deviceStatus.textContent = 'Reconnecting...';
      setTimeout(() => {
        deviceWs = null;
        openDeviceSocket();
      }, 2000);
    };
  }

  function ensureDeviceSocket() {
    if (deviceWs && (deviceWs.readyState === WebSocket.OPEN || deviceWs.readyState === WebSocket.CONNECTING)) {
      return;
    }
    openDeviceSocket();
  }

  function restartDeviceSocket() {
    if (deviceWs) {
      deviceWs.onclose = null;
      deviceWs.close();
      deviceWs = null;
    }
    openDeviceSocket();
  }

  async function setDeviceFilter(toggles, keepView) {
    if (!toggles.length) return;
    deviceStatus.textContent = 'Saving filters...';
    try {
      const res = await fetch('/api/device-filters', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ devices: toggles }),
      });
      if (!res.ok) throw new Error('device filter update failed');
      devicesState = devicesState.map((dev) => {
        const found = toggles.find((t) => t.mac === dev.mac);
        return found ? { ...dev, allowed: found.allowed } : dev;
      });
      if (!keepView) renderDevices(devicesState);
      deviceStatus.textContent = '';
    } catch (err) {
      deviceStatus.textContent = 'Unable to update device filters';
    }
  }

  function toggleAllDevices(allowed) {
    if (!devicesState.length) {
      deviceStatus.textContent = 'No devices to toggle';
      return;
    }
    const toggles = devicesState.map((dev) => ({ mac: dev.mac, allowed }));
    setDeviceFilter(toggles, false);
  }

  async function updateSound() {
    soundStatus.textContent = 'Saving sound preferences...';
    try {
      const res = await fetch('/api/sound', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          audio_jack: audioJack.checked,
          web_ui: webUi.checked,
          volume_by_signal: volumeBySignal.checked,
        }),
      });
      if (!res.ok) throw new Error('sound update failed');
      const data = await res.json();
      audioJack.checked = !!data.audio_jack;
      webUi.checked = !!data.web_ui_sound;
      soundStatus.textContent = 'Sound preferences saved';
      handleWebUiToggle();
    } catch (err) {
      soundStatus.textContent = 'Unable to save sound settings';
    }
  }

  async function savePackets() {
    packetStatus.textContent = 'Saving packet sounds...';
    try {
      const res = await fetch('/api/events', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          mode: modeSelect?.value,
          events: packetsState.map((p) => ({ id: p.id, enabled: !!p.enabled })),
        }),
      });
      if (!res.ok) throw new Error('packet update failed');
      const data = await res.json();
      packetsState = data.events || packetsState;
      packetStatus.textContent = `Saved (data tick every ${data.data_tick_n} frames)`;
    } catch (err) {
      packetStatus.textContent = 'Unable to save packet sounds';
    }
  }

  function handleWebUiToggle() {
    if (webUi.checked) {
      ensureWebsocket();
    } else {
      closeWebsocket();
    }
  }

  function ensureWebsocket() {
    if (ws && (ws.readyState === WebSocket.OPEN || ws.readyState === WebSocket.CONNECTING)) {
      return;
    }
    const proto = location.protocol === 'https:' ? 'wss' : 'ws';
    ws = new WebSocket(`${proto}://${location.host}/ws/packets`);
    ws.onmessage = (evt) => {
      try {
        const data = JSON.parse(evt.data);
        playEventSound(data.kind, !!data.retry, data.amplitude ?? 1);
      } catch {
        playEventSound('data-tick', false, 1);
      }
    };
    ws.onerror = () => ws && ws.close();
    ws.onclose = () => {
      if (webUi.checked) {
        setTimeout(ensureWebsocket, 1500);
      }
    };
  }

  function closeWebsocket() {
    if (ws) {
      ws.onclose = null;
      ws.close();
      ws = null;
    }
  }

  function playTick() {
    playEventSound('data-tick', false, 1);
  }

  function playEventSound(kind, retry, amplitude = 1) {
    if (!audioCtx) {
      const Ctx = window.AudioContext || window.webkitAudioContext;
      if (!Ctx) return;
      audioCtx = new Ctx();
    }
    const now = audioCtx.currentTime;
    const palette = {
      'beacon': { freq: 660, dur: 0.04, vol: 0.1 },
      'probe-req': { freq: 1200, dur: 0.04, vol: 0.13 },
      'probe-resp': { freq: 960, dur: 0.05, vol: 0.12 },
      'assoc': { freq: [520, 840], dur: 0.05, vol: 0.14 },
      'deauth': { noise: true, dur: 0.04, vol: 0.2 },
      'eapol': { seq: [640, 760, 880, 1020], dur: 0.03, vol: 0.12 },
      'rts': { freq: 360, dur: 0.03, vol: 0.12 },
      'cts': { freq: 480, dur: 0.03, vol: 0.12 },
      'ack': { freq: 2200, dur: 0.02, vol: 0.05 },
      'data-tick': { freq: 820, dur: 0.03, vol: 0.09 },
    };
    const entry = palette[kind] || palette['data-tick'];
    const gainScale = Math.max(0.1, Math.min(1.2, amplitude || 1));
    const gain = audioCtx.createGain();
    gain.gain.setValueAtTime((entry.vol || 0.1) * gainScale, now);
    gain.connect(audioCtx.destination);

    if (entry.noise) {
      const buffer = audioCtx.createBuffer(1, audioCtx.sampleRate * entry.dur, audioCtx.sampleRate);
      const data = buffer.getChannelData(0);
      for (let i = 0; i < data.length; i++) {
        const env = 1 - i / data.length;
        data[i] = (Math.random() * 2 - 1) * env * (entry.vol || 0.1);
      }
      const src = audioCtx.createBufferSource();
      src.buffer = buffer;
      src.connect(gain);
      src.start(now);
    } else if (entry.seq) {
      let offset = 0;
      entry.seq.forEach((freq) => {
        const osc = audioCtx.createOscillator();
        osc.frequency.value = freq;
        const g = audioCtx.createGain();
        g.gain.setValueAtTime((entry.vol || 0.1) * gainScale, now + offset);
        g.gain.exponentialRampToValueAtTime(0.0001, now + offset + entry.dur);
        osc.connect(g).connect(gain);
        osc.start(now + offset);
        osc.stop(now + offset + entry.dur);
        offset += entry.dur * 0.8;
      });
    } else if (entry.freq && Array.isArray(entry.freq)) {
      let offset = 0;
      entry.freq.forEach((freq) => {
        const osc = audioCtx.createOscillator();
        osc.frequency.value = freq;
        const g = audioCtx.createGain();
        g.gain.setValueAtTime((entry.vol || 0.1) * gainScale, now + offset);
        g.gain.exponentialRampToValueAtTime(0.0001, now + offset + entry.dur);
        osc.connect(g).connect(gain);
        osc.start(now + offset);
        osc.stop(now + offset + entry.dur);
        offset += entry.dur * 0.8;
      });
    } else {
      const osc = audioCtx.createOscillator();
      osc.frequency.value = entry.freq || 880;
      gain.gain.setValueAtTime((entry.vol || 0.1) * gainScale, now);
      gain.gain.exponentialRampToValueAtTime(0.0001, now + entry.dur);
      osc.connect(gain);
      osc.start(now);
      osc.stop(now + entry.dur);
    }

    if (retry) {
      const glitch = audioCtx.createGain();
      glitch.gain.setValueAtTime(0.05 * gainScale, now);
      glitch.gain.linearRampToValueAtTime(0.0, now + 0.06);
      const buffer = audioCtx.createBuffer(1, audioCtx.sampleRate * 0.02, audioCtx.sampleRate);
      const data = buffer.getChannelData(0);
      for (let i = 0; i < data.length; i++) {
        data[i] = (Math.random() * 2 - 1) * (1 - i / data.length);
      }
      const src = audioCtx.createBufferSource();
      src.buffer = buffer;
      src.connect(glitch).connect(audioCtx.destination);
      src.start(now);
    }
  }

  function toggle(show) {
    if (!modal) return;
    modal.classList.toggle('hidden', !show);
    if (!show && shutdownStatus) shutdownStatus.textContent = '';
  }

  open?.addEventListener('click', () => toggle(true));
  cancel?.addEventListener('click', () => toggle(false));

  confirm?.addEventListener('click', async () => {
    if (!confirm) return;
    confirm.disabled = true;
    shutdownStatus.textContent = 'Requesting shutdown...';
    try {
      const res = await fetch('/api/shutdown', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ confirm: true })
      });
      if (res.ok) {
        shutdownStatus.textContent = 'Shutdown requested';
      } else {
        shutdownStatus.textContent = 'Failed to request shutdown';
      }
    } catch (err) {
      shutdownStatus.textContent = 'Error while requesting shutdown';
    } finally {
      confirm.disabled = false;
    }
  });

  audioJack?.addEventListener('change', updateSound);
  webUi?.addEventListener('change', updateSound);
  volumeBySignal?.addEventListener('change', updateSound);
  modeSelect?.addEventListener('change', savePackets);
  toggleAll?.addEventListener('click', () => {
    const allOn = packetsState.every((p) => !!p.enabled);
    packetsState = packetsState.map((p) => ({ ...p, enabled: !allOn }));
    renderPackets(packetsState);
    savePackets();
  });
  deviceRefresh?.addEventListener('click', fetchDevices);
  deviceWindow?.addEventListener('change', () => {
    fetchDevices();
    restartDeviceSocket();
  });
  deviceSelectAll?.addEventListener('click', () => toggleAllDevices(true));
  deviceDeselectAll?.addEventListener('click', () => toggleAllDevices(false));
  deviceReset?.addEventListener('click', async () => {
    deviceStatus.textContent = 'Resetting counts...';
    try {
      const res = await fetch('/api/device-reset', { method: 'POST' });
      if (!res.ok) throw new Error('reset failed');
      deviceStatus.textContent = 'Counts reset';
      fetchDevices();
    } catch (err) {
      deviceStatus.textContent = 'Failed to reset counts';
    }
  });

  fetchSettings();
  fetchDevices();
  ensureDeviceSocket();
  setSection('channels');
})();
"#;

    rsx! {
        div { class: "page",
            meta { name: "viewport", content: "width=device-width, initial-scale=1" }
            div { class: "shell",
                div { class: "header",
                    h1 { class: "title", "Radioscope" }
                    p { class: "subtitle", "Wi-Fi packet to audio monitor" }
                    div { class: "tag", "Listening on {props.monitor_interface}" }
                }
                div { class: "nav",
                    button { class: "nav-btn active", "data-target": "channels", "Channels" }
                    button { class: "nav-btn", "data-target": "devices", "Devices" }
                    button { class: "nav-btn", "data-target": "packets", "Packet Types" }
                    button { class: "nav-btn", "data-target": "sound", "Sound" }
                    button { class: "nav-btn", "data-target": "system", "System" }
                }
                div { class: "content",
                    div { id: "section-channels", class: "card section active", "data-section": "channels",
                        h2 { class: "card-title", "Channel select" }
                        p { class: "muted", "Pick a Wi-Fi channel for the monitor interface. Channels are grouped by band." }
                        div { class: "channel-groups",
                            div { class: "group",
                                p { class: "group-title", "2.4 GHz" }
                                div { id: "channels-24", class: "channel-buttons" }
                            }
                            div { class: "group",
                                p { class: "group-title", "5 GHz" }
                                div { id: "channels-5", class: "channel-buttons" }
                            }
                        }
                        div { id: "channel-status", class: "status" }
                    }
                    div { id: "section-devices", class: "card section", "data-section": "devices",
                        h2 { class: "card-title", "Devices" }
                        p { class: "muted", "MAC addresses seen recently on the monitor interface. Adjust the lookback window and filter devices in or out." }
                        div { class: "device-controls",
                            label { class: "device-window",
                                span { "Window (min)" }
                                input { id: "device-window", r#type: "number", min: "1", max: "120", value: "10" }
                            }
                            button { id: "devices-refresh", class: "pill-btn", "Refresh" }
                            button { id: "devices-reset", class: "pill-btn", "Reset frame counts" }
                        }
                        div { class: "device-actions",
                            button { id: "devices-select-all", class: "pill-btn", "Select all" }
                            button { id: "devices-deselect-all", class: "pill-btn", "Deselect all" }
                        }
                        div { id: "device-list", class: "device-groups" }
                        div { id: "device-status", class: "status" }
                    }
                    div { id: "section-packets", class: "card section", "data-section": "packets",
                        h2 { class: "card-title", "Packet types" }
                        p { class: "muted", "Choose which packet events play sounds. Applies to both 3.5mm and Web UI audio." }
                        div { class: "mode-row",
                            label { "Mode:" }
                            select { id: "mode-select", class: "mode-select",
                                option { value: "crowded", "Crowded" }
                                option { value: "sparse", "Sparse" }
                            }
                        }
                        div { class: "packet-actions",
                            button { id: "packets-toggle-all", class: "pill-btn", "Select all" }
                        }
                        div { id: "packet-list", class: "packet-list" }
                        div { id: "packet-status", class: "status" }
                    }
                    div { id: "section-sound", class: "card section", "data-section": "sound",
                        h2 { class: "card-title", "Sound configuration" }
                        p { class: "muted", "Choose where packet ticks should play." }
                        div { class: "sound-options",
                            label { class: "checkbox",
                                input { id: "audio-jack", r#type: "checkbox" }
                                span { "3.5 mm audio jack" }
                            }
                            label { class: "checkbox",
                                input { id: "web-ui", r#type: "checkbox" }
                                span { "Web UI" }
                            }
                            label { class: "checkbox",
                                input { id: "volume-by-signal", r#type: "checkbox" }
                                span { "Volume follows signal strength" }
                            }
                        }
                        p { class: "caption", "When Web UI is on, ticks play in your browser via WebSocket notifications." }
                        div { id: "sound-status", class: "status" }
                    }
                    div { id: "section-system", class: "card section", "data-section": "system",
                        h2 { class: "card-title", "System" }
                        p { class: "muted", "Power controls for the device." }
                        button { id: "shutdown-btn", class: "primary", "Shutdown" }
                    }
                }
            }
        }
        div { id: "shutdown-modal", class: "modal hidden",
            div { class: "modal-card",
                h2 { class: "modal-title", "Confirm shutdown?" }
                p { class: "modal-body", "The device will power off immediately." }
                div { class: "actions",
                    button { id: "cancel-btn", class: "ghost", "Cancel" }
                    button { id: "confirm-btn", class: "danger", "Shutdown now" }
                }
                div { id: "status", class: "status" }
            }
        }
        style { "{styles}" }
        script { "{script}" }
    }
}
