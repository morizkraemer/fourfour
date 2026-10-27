<script>
  import './checkbox.css';

  /** @type {{ on?: boolean, label?: string, disabled?: boolean, onChange?: (on: boolean) => void }} */
  let { on = false, label = '', disabled = false, onChange } = $props();

  function toggle() {
    if (!disabled) onChange?.(!on);
  }

  function onKeydown(e) {
    if (e.key === ' ' || e.key === 'Enter') {
      e.preventDefault();
      toggle();
    }
  }
</script>

<span
  class="ff-toggle"
  class:ff-toggle--on={on}
  class:ff-toggle--disabled={disabled}
  role="switch"
  aria-checked={on}
  aria-disabled={disabled}
  aria-label={label || undefined}
  tabindex={disabled ? -1 : 0}
  onclick={toggle}
  onkeydown={onKeydown}
>
  <span class="ff-toggle__track"><span class="ff-toggle__knob"></span></span>
</span>
