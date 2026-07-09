DROP TABLE IF EXISTS `topic`;
CREATE TABLE IF NOT EXISTS `topic` (
  `topic_id` int NOT NULL DEFAULT '0',
  `topic_forum_id` int NOT NULL DEFAULT '0',
  `topic_name` varchar(255) NOT NULL DEFAULT '',
  `topic_date` bigint NOT NULL DEFAULT '0',
  `topic_ownername` varchar(255) NOT NULL DEFAULT '0',
  `topic_ownerid` int NOT NULL DEFAULT '0',
  `topic_type` int NOT NULL DEFAULT '0',
  `topic_reply` int NOT NULL DEFAULT '0'
) ;