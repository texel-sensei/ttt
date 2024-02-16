<script lang="ts">
  import { invoke } from '@tauri-apps/api/tauri';
  import type { Frame, Project } from '../backend';

  let errormessage: string|undefined = undefined;
  let frame: Frame|undefined = undefined;
  let project: Project|undefined = undefined;

  async function current() {
    try {
      frame = await invoke('current');
      console.log(frame);
      if (frame) {
        project = await invoke('lookup_project', {projectId: frame.project});
        console.log(project);
      }
    } catch (e: any) {
      errormessage = e.toString();
    }
  }
</script>

<div>
  <button on:click="{current}">Show</button>
  <p>{project?.name ?? 'Not running'}</p>
  <p>{frame?.start ?? 'Not running'}</p>
  <p>{errormessage ?? ''}</p>
</div>