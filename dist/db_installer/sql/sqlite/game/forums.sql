DROP TABLE IF EXISTS `forums`;
CREATE TABLE IF NOT EXISTS `forums` (
  `forum_id` int NOT NULL DEFAULT '0',
  `forum_name` varchar(255) NOT NULL DEFAULT '',
  `forum_parent` int NOT NULL DEFAULT '0',
  `forum_post` int NOT NULL DEFAULT '0',
  `forum_type` int NOT NULL DEFAULT '0',
  `forum_perm` int NOT NULL DEFAULT '0',
  `forum_owner_id` int NOT NULL DEFAULT '0',
  PRIMARY KEY (`forum_id`)
) ;

INSERT OR IGNORE INTO `forums` VALUES
(1, 'NormalRoot', 0, 0, 0, 1, 0),
(2, 'ClanRoot', 0, 0, 0, 0, 0),
(3, 'MemoRoot', 0, 0, 0, 0, 0),
(4, 'MailRoot', 0, 0, 0, 0, 0);
CREATE INDEX IF NOT EXISTS `idx_forums_owner_id` ON `forums` (`forum_owner_id`);
