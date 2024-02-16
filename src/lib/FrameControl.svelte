<script lang="ts">
  import { invoke } from '@tauri-apps/api/tauri';
  import type { Frame, Project } from '../backend';

  let errormessage: string|undefined = undefined;
  let frame: Frame|undefined = undefined;
  let project: Project|undefined = undefined;

  async function stop() {
    try {
      frame = await invoke('stop');
      console.log(frame);
      if (frame) {
        project = await invoke('lookup_project', {projectId: frame.project});
        console.log('Stopped project: ' + project);
        errormessage = undefined;
      } else {
        errormessage = 'Don\'t stop me now!';
      }
    } catch (e: any) {
      errormessage = e.toString();
    }
  }
</script>

<div>
  <button on:click="{stop}">Stop</button>
{#if errormessage}
{errormessage}
{/if}
</div>