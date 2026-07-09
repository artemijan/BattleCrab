DROP TABLE IF EXISTS `messages`;
CREATE TABLE IF NOT EXISTS `messages` (
  `messageId` INT NOT NULL DEFAULT 0,
  `senderId` INT NOT NULL DEFAULT 0,
  `receiverId` INT NOT NULL DEFAULT 0,
  `subject` TINYTEXT,
  `content` TEXT,
  `expiration` bigint NOT NULL DEFAULT '0',
  `reqAdena` BIGINT NOT NULL DEFAULT 0,
  `hasAttachments` varchar(10) DEFAULT 'false' NOT NULL,
  `isUnread` varchar(10) DEFAULT 'true' NOT NULL,
  `isDeletedBySender` varchar(10) DEFAULT 'false' NOT NULL,
  `isDeletedByReceiver` varchar(10) DEFAULT 'false' NOT NULL,
  `isLocked` varchar(10) DEFAULT 'false' NOT NULL,
  `sendBySystem` tinyint NOT NULL DEFAULT 0,
  `isReturned` varchar(10) DEFAULT 'false' NOT NULL,
  `itemId` INT NOT NULL DEFAULT '0',
  `enchantLvl` INT NOT NULL DEFAULT '0',
  `elementals` VARCHAR(25),
  PRIMARY KEY (`messageId`)
) ;