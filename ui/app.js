// The interface of the sample Studio "Ghi chú". Everything goes through the
// core: the page reads with read actions, writes with write actions, and
// refreshes when the core reports a change (made here or by the agent).
const studio = window.lynaxStudio;
const texts = {
  vi: { title: 'Ghi chú', add: 'Thêm', undo: 'Hoàn tác', redo: 'Làm lại', ask: 'Nhờ LynaX đặt lại tên', proposals: 'Đề xuất chờ duyệt',
        accept: 'Chấp nhận', dismiss: 'Bỏ', placeholder: 'Tiêu đề ghi chú', revision: 'Phiên bản {n}', select: 'Chọn',
        request: 'Đề xuất một tên rõ ràng hơn cho ghi chú “{title}”.', noBridge: 'Trang này cần chạy trong LynaX Desktop.',
        export: 'Xuất ra thư mục' },
  en: { title: 'Notes', add: 'Add', undo: 'Undo', redo: 'Redo', ask: 'Ask LynaX for a better name', proposals: 'Proposals to review',
        accept: 'Accept', dismiss: 'Dismiss', placeholder: 'Note title', revision: 'Revision {n}', select: 'Select',
        request: 'Suggest a clearer name for the note “{title}”.', noBridge: 'This page runs inside LynaX Desktop.',
        export: 'Export to folder' },
};
const format = (template, values) => template.replace(/\{(\w+)\}/g, (_, key) => values[key] ?? '');
// LynaX says which language (one this Studio lists) and theme to show, and when they change.
let text = texts.vi;
function applyEnvironment({ locale, theme }) {
  text = texts[locale] ?? texts.vi;
  document.documentElement.lang = locale;
  document.documentElement.style.colorScheme = theme;
  for (const node of document.querySelectorAll('[data-text]')) node.textContent = text[node.dataset.text];
  document.getElementById('note-title').placeholder = text.placeholder;
}
applyEnvironment(studio?.environment ?? { locale: 'vi', theme: 'light' });
/** What the person set for this Studio in LynaX (defaults applied by LynaX). */
let settings = { sort: 'position', show_revision: true };

const view = { notes: [], proposals: [], revision: 0, selected: null };
const errorBox = document.getElementById('error');
const fail = (error) => { errorBox.textContent = error?.message ?? String(error); };
const run = (action, args) => studio.invoke(action, args).then((result) => { errorBox.textContent = ''; return result; });

function button(label, onClick) {
  const element = document.createElement('button');
  element.type = 'button';
  element.textContent = label;
  element.addEventListener('click', () => onClick().catch(fail));
  return element;
}

function render() {
  const revision = document.getElementById('revision');
  revision.hidden = settings.show_revision === false;
  revision.textContent = format(text.revision, { n: view.revision });
  const notes = document.getElementById('notes');
  notes.replaceChildren(...view.notes.map((note) => {
    const row = document.createElement('li');
    row.dataset.id = note.id;
    row.setAttribute('aria-selected', String(view.selected === note.id));
    const title = document.createElement('span');
    title.textContent = note.title;
    row.append(title, button(text.select, () => select(note)));
    return row;
  }));
  const proposals = document.getElementById('proposals');
  document.getElementById('proposals-section').hidden = view.proposals.length === 0;
  proposals.replaceChildren(...view.proposals.map((proposal) => {
    const row = document.createElement('li');
    row.dataset.id = proposal.id;
    const summary = document.createElement('span');
    summary.textContent = proposal.summary;
    row.append(summary,
      button(text.accept, () => run('proposals.accept', { proposal_id: proposal.id })),
      button(text.dismiss, () => run('proposals.dismiss', { proposal_id: proposal.id })));
    return row;
  }));
}

async function refresh() {
  const [notes, proposals] = await Promise.all([run('notes.list'), run('proposals.list')]);
  Object.assign(view, { notes: notes.notes, revision: notes.revision, proposals: proposals.proposals });
  render();
}

async function select(note) {
  view.selected = note.id;
  render();
  await studio.context.set({ kind: 'note', id: note.id, label: note.title });
}

if (!studio) {
  errorBox.textContent = text.noBridge;
} else {
  document.getElementById('add').addEventListener('submit', (event) => {
    event.preventDefault();
    const input = document.getElementById('note-title');
    run('notes.add', { title: input.value }).then(() => { input.value = ''; }, fail);
  });
  document.getElementById('undo').addEventListener('click', () => run('history.undo').catch(fail));
  document.getElementById('redo').addEventListener('click', () => run('history.redo').catch(fail));
  document.getElementById('ask').addEventListener('click', () => {
    const note = view.notes.find((item) => item.id === view.selected) ?? view.notes[0];
    if (note) studio.ask(format(text.request, { title: note.title })).catch(fail);
  });
  // The export folder is the person's: asked for the first time it is needed.
  document.getElementById('export').addEventListener('click', async () => {
    try {
      const folder = (await studio.folders.list()).find((f) => f.key === 'exports');
      if (!folder?.granted && !(await studio.folders.request('exports'))) return;
      const done = await run('notes.export');
      errorBox.textContent = done.summary;
    } catch (error) {
      fail(error);
    }
  });
  studio.on('changed', () => refresh().catch(fail));
  studio.on('environment', (environment) => {
    applyEnvironment(environment);
    render();
  });
  // The core orders the notes by the settings it receives with each call: read them again.
  studio.on('settings', (values) => {
    settings = values;
    refresh().catch(fail);
  });
  window.notesReady = studio.settings.get().then((values) => { settings = values; }).then(refresh).catch(fail);
  window.notesView = () => view;
}
