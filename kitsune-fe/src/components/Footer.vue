<template>
  <footer>
    <div v-if="instanceData" class="footer">
      <span>Kitsune v{{ instanceData.instance.version }}</span>
      <a target="_blank" href="https://github.com/kitsune-soc/kitsune"
        >Source code</a
      >
    </div>
  </footer>
</template>

<script setup lang="ts">
  import { useInstanceInfo } from '../graphql/instance-info';
  const instanceData = useInstanceInfo();
</script>

<style scoped lang="scss">
  @use '../styles/colours' as *;

  .footer {
    display: flex;
    justify-content: center;
    align-items: flex-end;
    padding: 10px 0;
    gap: 25px;
    margin-bottom: 25px;
  }
</style>
