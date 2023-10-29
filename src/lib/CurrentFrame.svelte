<script>
  import { invoke } from '@tauri-apps/api/tauri'

  let errormessage = ''
  let frame = ''

  async function current() {
    try {
      frame = await invoke('current');
      console.log(frame);
    } catch (e) {
      errormessage = e.toString();
    }
  }
</script>

<div>
  <p>{frame}</p>
  <button on:click="{current}">Show</button>
  <p>{errormessage}</p>
</div>