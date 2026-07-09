DROP TABLE IF EXISTS `pledge_applicant`;
CREATE TABLE IF NOT EXISTS `pledge_applicant` (
  `charId` int NOT NULL,
  `clanId` int NOT NULL,
  `karma` tinyint NOT NULL,
  `message` varchar(255) NOT NULL,
  PRIMARY KEY (`charId`,`clanId`)
) ;

DROP TABLE IF EXISTS `pledge_recruit`;
CREATE TABLE IF NOT EXISTS `pledge_recruit` (
  `clan_id` int NOT NULL,
  `karma` tinyint NOT NULL,
  `information` varchar(50) NOT NULL,
  `detailed_information` varchar(255) NOT NULL,
  `application_type` tinyint NOT NULL,
  `recruit_type` tinyint NOT NULL
) ;

DROP TABLE IF EXISTS `pledge_waiting_list`;
CREATE TABLE IF NOT EXISTS `pledge_waiting_list` (
  `char_id` int NOT NULL,
  `karma` tinyint NOT NULL
) ;