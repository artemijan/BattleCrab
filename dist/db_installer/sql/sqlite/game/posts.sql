DROP TABLE IF EXISTS `posts`;
CREATE TABLE IF NOT EXISTS `posts` (
  `post_id` int NOT NULL DEFAULT '0',
  `post_owner_name` varchar(255) NOT NULL DEFAULT '',
  `post_ownerid` int NOT NULL DEFAULT '0',
  `post_date` bigint NOT NULL DEFAULT '0',
  `post_topic_id` int NOT NULL DEFAULT '0',
  `post_forum_id` int NOT NULL DEFAULT '0',
  `post_txt` text NOT NULL
) ;
CREATE INDEX IF NOT EXISTS `post_forum_id` ON `posts` (`post_forum_id`);