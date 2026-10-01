(() => {
  const rows = [...document.querySelectorAll('tr.test-result')];
  const search = document.querySelector('#test-search'), status = document.querySelector('#status-filter');
  const project = document.querySelector('#project-filter'), size = document.querySelector('#page-size');
  const count = document.querySelector('#result-count'), previous = document.querySelector('#previous-page'), next = document.querySelector('#next-page');
  const projects = new Map(rows.map(row => [row.dataset.project, row.dataset.projectName]));
  for (const [key, label] of [...projects].sort((a, b) => a[1].localeCompare(b[1]))) {
    const option = document.createElement('option'); option.value = key; option.textContent = label; project.append(option);
  }
  let page = 0;
  function update(reset = false) {
    if (reset) page = 0;
    const query = search.value.toLocaleLowerCase();
    const selected = rows.filter(row => row.dataset.name.toLocaleLowerCase().includes(query)
      && (!project.value || row.dataset.project === project.value)
      && (!status.value || (status.value === 'flaky' ? row.dataset.flaky === 'true' : row.dataset.status === status.value)));
    const limit = Number(size.value), pages = Math.max(1, Math.ceil(selected.length / limit));
    page = Math.min(page, pages - 1);
    for (const row of rows) row.hidden = true;
    for (const row of selected.slice(page * limit, (page + 1) * limit)) row.hidden = false;
    count.textContent = `${selected.length} of ${rows.length} tests · page ${page + 1} of ${pages}`;
    previous.disabled = page === 0; next.disabled = page + 1 >= pages;
    document.querySelector('#no-results').hidden = selected.length !== 0;
  }
  search.addEventListener('input', () => update(true));
  for (const control of [status, project, size]) control.addEventListener('change', () => update(true));
  previous.addEventListener('click', () => { page--; update(); });
  next.addEventListener('click', () => { page++; update(); });
  document.querySelector('#clear-filters').addEventListener('click', () => {
    search.value = ''; status.value = ''; project.value = ''; update(true); search.focus();
  });
  update();
})();
