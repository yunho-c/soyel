import { Select as SelectPrimitive } from "bits-ui";

import Content from "./select-content.svelte";
import Item from "./select-item.svelte";
import Trigger from "./select-trigger.svelte";

const Root = SelectPrimitive.Root;
const Group = SelectPrimitive.Group;
const Value = SelectPrimitive.Value;

export {
  Root,
  Group,
  Value,
  Trigger,
  Content,
  Item,
  Root as Select,
  Group as SelectGroup,
  Value as SelectValue,
  Trigger as SelectTrigger,
  Content as SelectContent,
  Item as SelectItem,
};
