<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";

  let version = $state("");
  invoke<string>("app_version").then((v) => (version = v));
</script>

<main>
  <h1>Bahi-Khata</h1>
  <p>Skeleton build {version}</p>
</main>
