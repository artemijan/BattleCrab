DROP TABLE IF EXISTS `items`;
CREATE TABLE IF NOT EXISTS `items` (
  `owner_id` INT, -- object id of the player or clan,owner of this item
  `object_id` INT NOT NULL DEFAULT 0, -- object id of the item
  `item_id` INT,
  `count` BIGINT NOT NULL DEFAULT 0,
  `enchant_level` INT,
  `loc` VARCHAR(10), -- inventory,paperdoll,npc,clan warehouse,pet,and so on
  `loc_data` INT,    -- depending on location: equiped slot,npc id,pet id,etc
  `time_of_use` INT, -- time of item use, for calculate of breackages
  `custom_type1` INT DEFAULT 0,
  `custom_type2` INT DEFAULT 0,
  `mana_left` decimal(5,0) NOT NULL DEFAULT -1,
  `time` decimal(13) NOT NULL DEFAULT 0,
  PRIMARY KEY (`object_id`)
) ;

CREATE INDEX IF NOT EXISTS idx_item_id ON items (item_id);
CREATE INDEX IF NOT EXISTS idx_object_id ON items (object_id);
CREATE INDEX IF NOT EXISTS idx_owner_id ON items (owner_id);
CREATE INDEX IF NOT EXISTS idx_owner_id_loc ON items (owner_id, loc);
CREATE INDEX IF NOT EXISTS idx_owner_id_item_id ON items (owner_id, item_id);
CREATE INDEX IF NOT EXISTS idx_owner_id_loc_locdata ON items (owner_id, loc, loc_data);
CREATE INDEX IF NOT EXISTS idx_owner_id_loc_locdata_enchant ON items (owner_id, loc, loc_data, enchant_level, item_id, object_id);
CREATE INDEX IF NOT EXISTS `owner_id` ON `items` (`owner_id`);
CREATE INDEX IF NOT EXISTS `item_id` ON `items` (`item_id`);
CREATE INDEX IF NOT EXISTS `loc` ON `items` (`loc`);
CREATE INDEX IF NOT EXISTS `time_of_use` ON `items` (`time_of_use`);